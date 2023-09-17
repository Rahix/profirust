#![deny(unused_must_use)]
use crate::fdl::FdlApplication;
use crate::phy::ProfibusPhy;

/// Operating state of the FDL master
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum ConnectivityState {
    /// The FDL master is not participating in bus communication in any way.
    Offline,
    /// The FDL master will respond to FDL status requests, but it does not want to become part of
    /// the token ring.
    Passive,
    /// The FDL master tries to enter the token ring to perform normal operations.
    Online,
}

impl ConnectivityState {
    #[inline(always)]
    pub fn is_offline(self) -> bool {
        self == ConnectivityState::Offline
    }

    #[inline(always)]
    pub fn is_passive(self) -> bool {
        self == ConnectivityState::Passive
    }

    #[inline(always)]
    pub fn is_online(self) -> bool {
        self == ConnectivityState::Online
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum GapState {
    /// Waiting for some time until the next gap polling cycle is performed.
    ///
    /// The `u8` value is the number of token rotations since the last polling cycle.
    Waiting(u8),

    /// A poll of the given address is scheduled next.
    NextPoll(u8),
}

impl GapState {
    pub fn increment_wait(&mut self) {
        match self {
            GapState::Waiting(ref mut r) => *r += 1,
            _ => (),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum CommunicationState {
    WithToken(StateWithToken),
    WithoutToken(StateWithoutToken),
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum StateWithToken {
    /// Master is ready to start a message cycle of any kind (unless it is waiting for the
    /// synchronization pause to pass).
    Idle { first: bool },
    /// Master should forward the token at the next possible time.
    ForwardToken,
    /// Waiting for the response to a message cycle.
    AwaitingResponse {
        addr: u8,
        sent_time: crate::time::Instant,
    },
    /// Waiting for the FDL status response of a potential peripheral.
    AwaitingFdlStatusResponse {
        addr: u8,
        sent_time: crate::time::Instant,
    },
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum StateWithoutToken {
    /// Master is idle.
    Idle,
    /// Waiting to respond to an FDL status request.
    PendingFdlStatusResponse {
        destination: u8,
        recv_time: crate::time::Instant,
    },
}

impl CommunicationState {
    pub fn have_token(&self) -> bool {
        matches!(self, CommunicationState::WithToken(_))
    }

    #[track_caller]
    pub fn assert_with_token(&mut self) -> &mut StateWithToken {
        if let CommunicationState::WithToken(s) = self {
            s
        } else {
            panic!("Expected to be holding the token at this time!");
        }
    }

    #[track_caller]
    pub fn assert_without_token(&mut self) -> &mut StateWithoutToken {
        if let CommunicationState::WithoutToken(s) = self {
            s
        } else {
            panic!("Expected to NOT be holding the token at this time!");
        }
    }
}

#[derive(Debug)]
pub struct FdlMaster {
    p: crate::fdl::Parameters,

    /// Address of the next master in the token ring.
    ///
    /// This value is always valid and will be our own station address when no other master is
    /// known.
    next_master: u8,

    /// Timestamp of the last time we found the bus to be active (= someone transmitting).
    ///
    /// Used for detecting various timeouts:
    ///
    /// - Token Lost
    /// - Peripheral not Responding
    last_bus_activity: Option<crate::time::Instant>,

    /// Amount of bytes pending in the receive buffer.
    ///
    /// This known value is compared to the latest one reported by the PHY to find out whether new
    /// data was received since the last poll.
    pending_bytes: u8,

    /// Whether we believe to be a part of the token ring.
    in_ring: bool,

    /// Timestamp of last token acquisition.
    last_token_time: Option<crate::time::Instant>,
    /// Timestamp of the second to last token acquisition.
    previous_token_time: Option<crate::time::Instant>,

    /// State of the gap polling machinery.
    gap_state: GapState,

    /// List of live stations.
    live_list: bitvec::BitArr!(for 256),

    /// State of the master.
    communication_state: CommunicationState,

    /// Operating State of the master.
    connectivity_state: ConnectivityState,
}

impl FdlMaster {
    /// Construct a new FDL master with the given parameters.
    pub fn new(param: crate::fdl::Parameters) -> Self {
        let mut live_list = bitvec::array::BitArray::ZERO;
        // Mark ourselves as "live".
        live_list.set(usize::from(param.address), true);

        debug_assert!(param.highest_station_address <= 126);

        Self {
            next_master: param.address,
            last_bus_activity: None,
            pending_bytes: 0,
            last_token_time: None,
            previous_token_time: None,
            gap_state: GapState::NextPoll(param.address.wrapping_add(1)),
            live_list,
            communication_state: CommunicationState::WithoutToken(StateWithoutToken::Idle),
            in_ring: false,
            connectivity_state: ConnectivityState::Offline,

            p: param,
        }
    }

    /// Return a reference to the parameters configured for this FDL master.
    #[inline(always)]
    pub fn parameters(&self) -> &crate::fdl::Parameters {
        &self.p
    }

    /// Returns `true` when this FDL master believes to be in the token ring.
    #[inline(always)]
    pub fn is_in_ring(&self) -> bool {
        self.in_ring
    }

    fn update_live_state(&mut self, addr: u8, live: bool) {
        let previous = *self.live_list.get(usize::from(addr)).unwrap();
        if live && !previous {
            log::debug!("Discovered device #{addr}.");
        } else if !live && previous {
            log::debug!("Lost contact with device #{addr}.");
        }
        self.live_list.set(usize::from(addr), live);
    }

    /// Returns `true` when the given address is believed to be "alive" (responds on the bus).
    pub fn check_address_live(&self, addr: u8) -> bool {
        *self
            .live_list
            .get(usize::from(addr))
            .expect("invalid address")
    }

    /// Iterator over all station addresses which are currently responding on the bus.
    pub fn iter_live_stations(&self) -> impl Iterator<Item = u8> + '_ {
        self.live_list
            .iter_ones()
            .map(|addr| u8::try_from(addr).unwrap())
    }

    #[inline(always)]
    pub fn connectivity_state(&self) -> ConnectivityState {
        self.connectivity_state
    }

    #[inline]
    pub fn set_state(&mut self, state: ConnectivityState) {
        log::info!("FDL master entering state \"{:?}\"", state);
        self.connectivity_state = state;

        if state == ConnectivityState::Offline {
            // If we are going offline, reset all internal state by recreating the FDL master.
            let parameters = core::mem::take(&mut self.p);
            *self = Self::new(parameters);
        } else if state != ConnectivityState::Online {
            todo!(
                "ConnectivityState {:?} is not yet supported properly!",
                state
            );
        }
    }

    /// Enter the [`Offline`][`ConnectivityState::Offline`] connectivity state.
    ///
    /// This is equivalent to calling `.set_state(ConnectivityState::Offline)`.
    #[inline]
    pub fn set_offline(&mut self) {
        self.set_state(ConnectivityState::Offline)
    }

    /// Enter the [`Passive`][`ConnectivityState::Passive`] connectivity state.
    ///
    /// This is equivalent to calling `.set_state(ConnectivityState::Passive)`.
    #[inline]
    pub fn set_passive(&mut self) {
        self.set_state(ConnectivityState::Passive)
    }

    /// Enter the [`Online`][`ConnectivityState::Online`] connectivity state.
    ///
    /// This is equivalent to calling `.set_state(ConnectivityState::Online)`.
    #[inline]
    pub fn set_online(&mut self) {
        self.set_state(ConnectivityState::Online)
    }
}

#[must_use = "\"poll done\" marker must lead to exit of poll function!"]
struct PollDone();

#[must_use = "\"poll result\" must lead to exit of poll function!"]
struct PollResult<E> {
    event: Option<E>,
}

impl PollDone {
    pub fn waiting_for_transmission() -> Self {
        PollDone()
    }

    pub fn waiting_for_bus() -> Self {
        PollDone()
    }

    pub fn waiting_for_delay() -> Self {
        PollDone()
    }

    pub fn offline() -> Self {
        PollDone()
    }

    pub fn with_event<E>(self, ev: E) -> PollResult<E> {
        PollResult { event: Some(ev) }
    }

    pub fn with_event_maybe<E>(self, event: Option<E>) -> PollResult<E> {
        PollResult { event }
    }
}

impl<E> From<PollDone> for PollResult<E> {
    fn from(value: PollDone) -> Self {
        PollResult { event: None }
    }
}

macro_rules! return_if_done {
    ($expr:expr) => {
        match $expr {
            Some(e) => return e.into(),
            None => (),
        }
    };
}

impl FdlMaster {
    /// Mark the bus as active at the current point in time.
    ///
    /// Sets the current time as last_bus_activity unless we have already deduced that bus activity
    /// will continue until some point in the future.
    fn mark_bus_activity(&mut self, now: crate::time::Instant) {
        let last = self.last_bus_activity.get_or_insert(now);
        *last = (*last).max(now);
    }

    /// Check whether a transmission is currently ongoing.
    ///
    /// There are two scenarios where an ongoing transmission is detected:
    ///
    /// 1. If the PHY reports that it is still transmitting.
    /// 2. If we believe that we must still be sending data from timing calculations.
    fn check_for_ongoing_transmision(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
    ) -> Option<PollDone> {
        if self.last_bus_activity.map(|l| now <= l).unwrap_or(false) || phy.is_transmitting() {
            self.mark_bus_activity(now);
            Some(PollDone::waiting_for_transmission())
        } else {
            None
        }
    }

    /// Wait for 33 bit times since last bus activity.
    ///
    /// This synchronization pause is required before every transmission.
    fn wait_synchronization_pause(&mut self, now: crate::time::Instant) -> Option<PollDone> {
        debug_assert!(self.communication_state.have_token());
        // TODO: Is it right to write the last_bus_activity here?  Probably does not matter as
        // handle_lost_token() will most likely get called way earlier.
        if now <= (*self.last_bus_activity.get_or_insert(now) + self.p.bits_to_time(33)) {
            Some(PollDone::waiting_for_delay())
        } else {
            None
        }
    }

    /// Check whether the time to respond has passed without initiation of a response.
    fn check_slot_expired(&mut self, now: crate::time::Instant) -> bool {
        // We have two situations:
        // 1. Either the slot expires without any repsonse activity at all
        // 2. Or we received some bytes, but not a full telegram
        let last_bus_activity = *self.last_bus_activity.get_or_insert(now);
        if self.pending_bytes == 0 {
            now > (last_bus_activity + self.p.slot_time())
        } else {
            // TODO: Technically, no inter-character delay is allowed at all but we are in a rough
            // spot here.  The peripheral will most likely continue transmitting data in a short
            // while so let's be conservative and wait an entire slot time again after partial
            // receival.
            //
            // The tricky part here is that this timeout also becomes very relevant on non-realtime
            // systems like a vanilla Linux where PROFIBUS communication happens over USB.  We can
            // have longer delays between consecutive characters there.  For example, sometimes
            // data is received in chunks of 32 bytes.  This obviously looks like a large
            // inter-character delay that we need to be robust against.
            now > (last_bus_activity + self.p.slot_time())
        }
    }

    /// Marks transmission starting `now` and continuing for `bytes` length.
    fn mark_tx(&mut self, now: crate::time::Instant, bytes: usize) -> PollDone {
        self.last_bus_activity = Some(
            now + self
                .p
                .baudrate
                .bits_to_time(11 * u32::try_from(bytes).unwrap()),
        );
        PollDone::waiting_for_transmission()
    }

    fn check_for_bus_activity(&mut self, now: crate::time::Instant, phy: &mut impl ProfibusPhy) {
        let pending_bytes = phy.get_pending_received_bytes().try_into().unwrap();
        if pending_bytes > self.pending_bytes {
            self.mark_bus_activity(now);
            self.pending_bytes = pending_bytes;
        }
    }
}

impl FdlMaster {
    fn acquire_token(&mut self, now: crate::time::Instant, token: &crate::fdl::TokenTelegram) {
        debug_assert!(token.da == self.p.address);
        self.communication_state =
            CommunicationState::WithToken(StateWithToken::Idle { first: true });
        self.previous_token_time = self.last_token_time;
        self.last_token_time = Some(now);
        self.gap_state.increment_wait();
        self.in_ring = true;
        log::trace!("{} acquired the token!", self.p.address);
    }

    #[must_use = "poll done marker"]
    fn forward_token(&mut self, now: crate::time::Instant, phy: &mut impl ProfibusPhy) -> PollDone {
        self.communication_state = CommunicationState::WithoutToken(StateWithoutToken::Idle);

        let token_telegram = crate::fdl::TokenTelegram::new(self.next_master, self.p.address);
        if self.next_master == self.p.address {
            // Special case when the token is also fowarded to ourselves.
            self.acquire_token(now, &token_telegram);
        }

        let tx_res = phy
            .transmit_telegram(|tx| Some(tx.send_token_telegram(self.next_master, self.p.address)))
            .unwrap();
        self.mark_tx(now, tx_res.bytes_sent())
    }

    #[must_use = "poll done marker"]
    fn handle_lost_token(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
    ) -> Option<PollDone> {
        // If we do not know of any previous bus activity, conservatively assume that the last
        // activity was just now and start counting from here...
        let last_bus_activity = *self.last_bus_activity.get_or_insert(now);
        if (now - last_bus_activity) >= self.p.token_lost_timeout() {
            if self.in_ring {
                log::warn!("Token lost! Generating a new one.");
            } else {
                log::info!("Generating new token due to silent bus.");
            }
            self.next_master = self.p.address;
            Some(self.forward_token(now, phy))
        } else {
            None
        }
    }

    #[must_use = "poll done marker"]
    fn app_transmit_telegram<APP: FdlApplication>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
        app: &mut APP,
        high_prio_only: bool,
    ) -> Option<PollResult<APP::Event>> {
        debug_assert!(self.communication_state.have_token());
        let mut event = None;
        if let Some(tx_res) = phy.transmit_telegram(|tx| {
            let (res, ev) = app.transmit_telegram(now, self, tx, high_prio_only);
            event = ev;
            res
        }) {
            if let Some(addr) = tx_res.expects_reply() {
                *self.communication_state.assert_with_token() = StateWithToken::AwaitingResponse {
                    addr,
                    sent_time: now,
                };
            }
            Some(
                self.mark_tx(now, tx_res.bytes_sent())
                    .with_event_maybe(event),
            )
        } else {
            None
        }
    }

    fn app_receive_reply<APP: FdlApplication>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
        app: &mut APP,
        addr: u8,
    ) -> Option<Option<APP::Event>> {
        phy.receive_telegram(|telegram| {
            match &telegram {
                crate::fdl::Telegram::Token(t) => {
                    log::warn!(
                        "Received token telegram {t:?} while waiting for peripheral response"
                    );
                    return None;
                }
                crate::fdl::Telegram::Data(t) => {
                    if t.is_response().is_none() {
                        log::warn!("Received non-response telegram: {t:?}");
                        return None;
                    }
                    if t.h.da != self.p.address {
                        log::warn!("Received telegram with unexpected destination: {t:?}");
                        return None;
                    }
                }
                crate::fdl::Telegram::ShortConfirmation(_) => (),
            }
            // TODO: This needs to be revisited.  Always return true?
            Some(app.receive_reply(now, self, addr, telegram))
        })
        .unwrap_or(None)
    }

    fn app_handle_timeout(
        &mut self,
        now: crate::time::Instant,
        app: &mut impl FdlApplication,
        addr: u8,
    ) {
        app.handle_timeout(now, self, addr)
    }

    fn check_for_status_response(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
        addr: u8,
    ) -> bool {
        phy.receive_telegram(|telegram| {
            if let crate::fdl::Telegram::Data(telegram) = telegram {
                if telegram.h.sa != addr {
                    log::warn!("Expected status response from {addr}, got telegram from someone else: {telegram:?}");
                    false
                } else if let crate::fdl::FunctionCode::Response { state, status } = telegram.h.fc {
                    log::trace!("Got status response from {addr}!");
                    if state == crate::fdl::ResponseState::MasterWithoutToken
                        || state == crate::fdl::ResponseState::MasterInRing
                    {
                        self.next_master = telegram.h.sa;
                    }
                    true
                } else {
                    log::warn!("Unexpected telegram while waiting for status response from {addr}: {telegram:?}");
                    false
                }
            } else {
                false
            }
        })
        .unwrap_or(false)
    }

    fn next_gap_poll(&self, addr: u8) -> GapState {
        if addr == self.next_master && addr != self.p.address {
            // Don't poll beyond the gap.
            GapState::Waiting(0)
        } else if (addr + 1) == self.p.address {
            // Don't poll self.
            GapState::Waiting(0)
        } else if addr == (self.p.highest_station_address - 1) {
            // Wrap around.
            GapState::NextPoll(0)
        } else {
            GapState::NextPoll(addr + 1)
        }
    }

    #[must_use = "poll done marker"]
    fn handle_gap(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
    ) -> Option<PollDone> {
        assert!(self.communication_state.have_token());

        if let GapState::Waiting(r) = self.gap_state {
            if r >= self.p.gap_wait_rotations {
                // We're done waiting, do a poll now!
                log::debug!("Starting next gap polling cycle!");
                self.gap_state = self.next_gap_poll(self.p.address);
            }
        }

        if let GapState::NextPoll(addr) = self.gap_state {
            self.gap_state = self.next_gap_poll(addr);

            *self.communication_state.assert_with_token() =
                StateWithToken::AwaitingFdlStatusResponse {
                    addr,
                    sent_time: now,
                };

            let tx_res = phy
                .transmit_telegram(|tx| Some(tx.send_fdl_status_request(addr, self.p.address)))
                .unwrap();
            return Some(self.mark_tx(now, tx_res.bytes_sent()));
        }

        None
    }

    #[must_use = "poll done marker"]
    fn handle_with_token<APP: FdlApplication>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
        app: &mut APP,
    ) -> PollResult<APP::Event> {
        // First check for ongoing message cycles and handle them.
        match *self.communication_state.assert_with_token() {
            StateWithToken::AwaitingResponse { addr, sent_time } => {
                if let Some(event) = self.app_receive_reply(now, phy, app, addr) {
                    *self.communication_state.assert_with_token() =
                        StateWithToken::Idle { first: false };
                    // Waiting for synchronization pause now
                    PollDone::waiting_for_delay().with_event_maybe(event)
                } else if self.check_slot_expired(now) {
                    self.app_handle_timeout(now, app, addr);
                    *self.communication_state.assert_with_token() =
                        StateWithToken::Idle { first: false };
                    // TODO: Will this transmit or wait?
                    self.handle_with_token_transmission(now, phy, app)
                } else {
                    // Still waiting for the response, nothing to do here.
                    PollDone::waiting_for_bus().into()
                }
            }
            StateWithToken::AwaitingFdlStatusResponse { addr, sent_time } => {
                if self.check_for_status_response(now, phy, addr) {
                    log::trace!("Address {addr} responded!");
                    // After the gap response, we pass on the token.
                    self.update_live_state(addr, true);
                    *self.communication_state.assert_with_token() = StateWithToken::ForwardToken;
                    // Waiting for synchronization pause now
                    PollDone::waiting_for_delay().into()
                } else if self.check_slot_expired(now) {
                    log::trace!("Address {addr} didn't respond in {}!", self.p.slot_time());
                    // Mark this address as not alive and pass on the token.
                    self.update_live_state(addr, false);
                    *self.communication_state.assert_with_token() = StateWithToken::ForwardToken;
                    // TODO: Will this transmit or wait?
                    self.handle_with_token_transmission(now, phy, app)
                } else {
                    // Still waiting for the response, nothing to do here.
                    PollDone::waiting_for_bus().into()
                }
            }
            StateWithToken::Idle { .. } | StateWithToken::ForwardToken => {
                self.handle_with_token_transmission(now, phy, app)
            }
        }
    }

    #[must_use = "poll done marker"]
    fn handle_with_token_transmission<APP: FdlApplication>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
        app: &mut APP,
    ) -> PollResult<APP::Event> {
        // Before we can send anything, we must always wait 33 bit times (synchronization pause).
        return_if_done!(self.wait_synchronization_pause(now));

        let first_with_token = match self.communication_state.assert_with_token() {
            StateWithToken::ForwardToken => {
                return self.forward_token(now, phy).into();
            }
            StateWithToken::Idle { first } => *first,
            _ => unreachable!(),
        };

        // Check if there is still time to start a message cycle.
        if let Some(rotation_time) = self.previous_token_time.map(|p| now - p) {
            if rotation_time >= self.p.token_rotation_time() {
                // If we're over the rotation time and just acquired the token, we are allowed to
                // perform one more high priority message cycle.
                if first_with_token {
                    return_if_done!(self.app_transmit_telegram(now, phy, app, true));
                }

                // In any other case, we pass on the token to the next master.
                return self.forward_token(now, phy).into();
            }
        }

        // We have time, try doing useful things.
        return_if_done!(self.app_transmit_telegram(now, phy, app, false));

        // If we end up here, there's nothing useful left to do so now handle the gap polling cycle.
        return_if_done!(self.handle_gap(now, phy));

        // And if even the gap poll didn't lead to a message, pass token immediately.
        self.forward_token(now, phy).into()
    }

    #[must_use = "poll done marker"]
    fn handle_without_token(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
    ) -> PollDone {
        debug_assert!(!self.communication_state.have_token());

        enum NextAction {
            RespondWithStatus { da: u8 },
        }

        match *self.communication_state.assert_without_token() {
            StateWithoutToken::Idle => phy
                .receive_telegram(|telegram| {
                    match telegram {
                        crate::fdl::Telegram::Token(token_telegram) => {
                            if token_telegram.da == self.p.address {
                                // Heyy, we got the token!
                                self.acquire_token(now, &token_telegram);
                                PollDone::waiting_for_delay()
                            } else {
                                log::trace!(
                                    "Witnessed token passing: {} => {}",
                                    token_telegram.sa,
                                    token_telegram.da,
                                );
                                if token_telegram.sa == token_telegram.da {
                                    if self.in_ring {
                                        log::info!(
                                            "Left the token ring due to self-passing by addr {}.",
                                            token_telegram.sa
                                        );
                                    }
                                    self.in_ring = false;
                                }
                                PollDone::waiting_for_bus()
                            }
                        }
                        crate::fdl::Telegram::Data(telegram) if telegram.h.da == self.p.address => {
                            if let Some(da) = telegram.is_fdl_status_request() {
                                *self.communication_state.assert_without_token() =
                                    StateWithoutToken::PendingFdlStatusResponse {
                                        destination: da,
                                        recv_time: now,
                                    };
                                PollDone::waiting_for_delay()
                            } else {
                                PollDone::waiting_for_bus()
                            }
                        }
                        t => {
                            // Unhandled telegram, probably not for us.
                            PollDone::waiting_for_bus()
                        }
                    }
                })
                .unwrap_or_else(|| {
                    // When we did not receive a telegram, check whether the token got lost and if
                    // it didn't, just end the poll cycle.
                    return_if_done!(self.handle_lost_token(now, phy));
                    PollDone::waiting_for_bus()
                }),
            StateWithoutToken::PendingFdlStatusResponse {
                destination,
                recv_time,
            } if (now - recv_time) >= self.p.min_tsdr_time() => {
                *self.communication_state.assert_without_token() = StateWithoutToken::Idle;

                let state = if self.in_ring {
                    crate::fdl::ResponseState::MasterInRing
                } else {
                    crate::fdl::ResponseState::MasterWithoutToken
                };

                let tx_res = phy
                    .transmit_telegram(|tx| {
                        Some(tx.send_fdl_status_response(
                            destination,
                            self.p.address,
                            state,
                            crate::fdl::ResponseStatus::Ok,
                        ))
                    })
                    .unwrap();
                self.mark_tx(now, tx_res.bytes_sent())
            }
            // Continue waiting...
            StateWithoutToken::PendingFdlStatusResponse { .. } => PollDone::waiting_for_delay(),
        }
    }

    pub fn poll<'a, PHY: ProfibusPhy, APP: FdlApplication>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
        app: &mut APP,
    ) -> Option<APP::Event> {
        let result = self.poll_inner(now, phy, app);
        if !phy.is_transmitting() {
            self.pending_bytes = phy.get_pending_received_bytes().try_into().unwrap();
        }
        result.event
    }

    fn poll_inner<'a, PHY: ProfibusPhy, APP: FdlApplication>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
        app: &mut APP,
    ) -> PollResult<APP::Event> {
        if self.connectivity_state == ConnectivityState::Offline {
            // When we are offline, don't do anything at all.
            return PollDone::offline().into();
        }

        return_if_done!(self.check_for_ongoing_transmision(now, phy));

        self.check_for_bus_activity(now, phy);

        if self.communication_state.have_token() {
            self.handle_with_token(now, phy, app)
        } else {
            self.handle_without_token(now, phy).into()
        }
    }
}

#[cfg(test)]
mod tests {
    /// Ensure the `FdlMaster` struct size doesn't completely get out of control.
    #[test]
    fn fdl_master_size() {
        assert!(std::mem::size_of::<crate::fdl::FdlMaster>() <= 256);
    }
}
