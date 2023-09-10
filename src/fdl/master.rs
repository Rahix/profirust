#![deny(unused_must_use)]
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

        debug_assert!(param.highest_station_address <= 125);

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

macro_rules! return_if_done {
    ($expr:expr) => {
        match $expr {
            e @ Some(PollDone()) => return e,
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
            Some(PollDone())
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
        if now <= (*self.last_bus_activity.get_or_insert(now) + self.p.baudrate.bits_to_time(33)) {
            Some(PollDone())
        } else {
            None
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
        PollDone()
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

    #[must_use = "tx token"]
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

    #[must_use = "tx token"]
    fn handle_lost_token(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
    ) -> Option<PollDone> {
        // If we do not know of any previous bus activity, conservatively assume that the last
        // activity was just now and start counting from here...
        let last_bus_activity = *self.last_bus_activity.get_or_insert(now);
        if (now - last_bus_activity) >= self.p.token_lost_timeout() {
            log::warn!("Token lost! Generating a new one.");
            self.next_master = self.p.address;
            Some(self.forward_token(now, phy))
        } else {
            None
        }
    }

    #[must_use = "tx token"]
    fn try_start_message_cycle(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
        app: &mut impl crate::fdl::FdlApplication,
        high_prio_only: bool,
    ) -> Option<PollDone> {
        debug_assert!(self.communication_state.have_token());
        if let Some(tx_res) =
            phy.transmit_telegram(|tx| app.transmit_telegram(now, self, tx, high_prio_only))
        {
            if let Some(addr) = tx_res.expects_reply() {
                *self.communication_state.assert_with_token() = StateWithToken::AwaitingResponse {
                    addr,
                    sent_time: now,
                };
            }
            Some(self.mark_tx(now, tx_res.bytes_sent()))
        } else {
            None
        }
    }

    fn check_for_response(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
        app: &mut impl crate::fdl::FdlApplication,
        addr: u8,
    ) -> bool {
        phy.receive_telegram(|telegram| {
            match &telegram {
                crate::fdl::Telegram::Token(t) => {
                    log::warn!(
                        "Received token telegram {t:?} while waiting for peripheral response"
                    );
                    return false;
                }
                crate::fdl::Telegram::Data(t) => {
                    if t.h.da != self.p.address {
                        log::warn!("Received telegram with unexpected destination: {t:?}");
                        return false;
                    }
                }
                _ => (),
            }
            // TODO: This needs to be revisited.  Always return true?
            app.receive_reply(now, self, addr, telegram);
            return true;
        })
        .unwrap_or(false)
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
        } else if addr == self.p.highest_station_address {
            // Wrap around.
            GapState::NextPoll(0)
        } else {
            GapState::NextPoll(addr + 1)
        }
    }

    #[must_use = "tx token"]
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

    #[must_use = "tx token"]
    fn handle_with_token(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
        app: &mut impl crate::fdl::FdlApplication,
    ) -> Option<PollDone> {
        // First check for ongoing message cycles and handle them.
        match *self.communication_state.assert_with_token() {
            StateWithToken::AwaitingResponse { addr, sent_time } => {
                if self.check_for_response(now, phy, app, addr) {
                    *self.communication_state.assert_with_token() =
                        StateWithToken::Idle { first: false };
                } else if (now - sent_time) >= self.p.slot_time() {
                    todo!("handle message cycle response timeout");
                } else {
                    // Still waiting for the response, nothing to do here.
                    // TODO: shouldn't we also return the marker here?
                    return None;
                }
            }
            StateWithToken::AwaitingFdlStatusResponse { addr, sent_time } => {
                if self.check_for_status_response(now, phy, addr) {
                    log::trace!("Address {addr} responded!");
                    // After the gap response, we pass on the token.
                    self.live_list.set(usize::from(addr), true);
                    *self.communication_state.assert_with_token() = StateWithToken::ForwardToken;
                } else if (now - sent_time) >= self.p.slot_time() {
                    log::trace!("Address {addr} didn't respond in {}!", self.p.slot_time());
                    // Mark this address as not alive and pass on the token.
                    self.live_list.set(usize::from(addr), false);
                    *self.communication_state.assert_with_token() = StateWithToken::ForwardToken;
                } else {
                    // Still waiting for the response, nothing to do here.
                    // TODO: shouldn't we also return the marker here?
                    return None;
                }
            }
            // Continue towards transmission when idle or forwarding token.
            StateWithToken::Idle { .. } => (),
            StateWithToken::ForwardToken => (),
        }

        // Before we can send anything, we must always wait 33 bit times (synchronization pause).
        return_if_done!(self.wait_synchronization_pause(now));

        let first_with_token = match self.communication_state.assert_with_token() {
            StateWithToken::ForwardToken => {
                return Some(self.forward_token(now, phy));
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
                    return_if_done!(self.try_start_message_cycle(now, phy, app, true));
                }

                // In any other case, we pass on the token to the next master.
                return Some(self.forward_token(now, phy));
            }
        }

        // We have time, try doing useful things.
        return_if_done!(self.try_start_message_cycle(now, phy, app, false));

        // If we end up here, there's nothing useful left to do so now handle the gap polling cycle.
        return_if_done!(self.handle_gap(now, phy));

        // And if even the gap poll didn't lead to a message, pass token immediately.
        return Some(self.forward_token(now, phy));
    }

    #[must_use = "tx token"]
    fn handle_without_token(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
    ) -> Option<PollDone> {
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
                            }
                            None
                        }
                        crate::fdl::Telegram::Data(telegram) if telegram.h.da == self.p.address => {
                            if let Some(da) = telegram.is_fdl_status_request() {
                                *self.communication_state.assert_without_token() =
                                    StateWithoutToken::PendingFdlStatusResponse {
                                        destination: da,
                                        recv_time: now,
                                    };
                            }
                            None
                        }
                        t => {
                            // Unhandled telegram, probably not for us.
                            None
                        }
                    }
                })
                .unwrap_or(None),
            // TODO: This should be min(Tsdr)
            StateWithoutToken::PendingFdlStatusResponse {
                destination,
                recv_time,
            } if (now - recv_time) >= self.p.bits_to_time(11) => {
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
                Some(self.mark_tx(now, tx_res.bytes_sent()))
            }
            // Continue waiting...
            StateWithoutToken::PendingFdlStatusResponse { .. } => None,
        }
    }

    pub fn poll<'a, PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
        app: &mut impl crate::fdl::FdlApplication,
    ) {
        let _ = self.poll_inner(now, phy, app);
        if !phy.is_transmitting() {
            self.pending_bytes = phy.get_pending_received_bytes().try_into().unwrap();
        }
    }

    fn poll_inner<'a, PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
        app: &mut impl crate::fdl::FdlApplication,
    ) -> Option<PollDone> {
        if self.connectivity_state == ConnectivityState::Offline {
            // When we are offline, don't do anything at all.
            return None;
        }

        return_if_done!(self.check_for_ongoing_transmision(now, phy));

        self.check_for_bus_activity(now, phy);

        if self.communication_state.have_token() {
            // log::trace!("{} has token!", self.p.address);
            return_if_done!(self.handle_with_token(now, phy, app));
        } else {
            return_if_done!(self.handle_without_token(now, phy));
            // We may have just received the token so do one more pass "with token".
            if self.communication_state.have_token() {
                return_if_done!(self.handle_with_token(now, phy, app));
            }
        }

        return_if_done!(self.handle_lost_token(now, phy));
        None
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
