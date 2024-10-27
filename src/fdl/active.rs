//! Implementation of an FDL active station.

#![deny(unused_must_use)]
use crate::fdl::FdlApplication;
use crate::phy::ProfibusPhy;

/// Operating state of the FDL active station
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum ConnectivityState {
    /// The station is not participating in bus communication in any way.
    Offline,
    /// The station is only passive.
    ///
    /// It will respond to requests, but it will not attempt to become part of the token ring.
    Passive,
    /// The station tries to become part of the token ring and then performs communication.
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
    /// The `rotation_count` value is the number of token rotations since the last polling cycle.
    Waiting { rotation_count: u8 },

    /// A poll of the given address is scheduled next.
    NextPoll { next_addr: crate::Address },
}

impl GapState {
    pub fn increment_wait(&mut self) {
        match self {
            GapState::Waiting {
                ref mut rotation_count,
            } => *rotation_count += 1,
            _ => (),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum State {
    Offline,
    PassiveIdle,
    ListenToken {
        status_request: Option<crate::Address>,
        collision_count: u8,
    },
    ActiveIdle {
        status_request: Option<crate::Address>,
    },
    UseToken,
    ClaimToken {
        first: bool,
    },
    AwaitDataResponse,
    PassToken {
        do_gap: bool,
    },
    CheckTokenPass,
    AwaitStatusResponse {
        address: crate::Address,
    },
}

macro_rules! debug_assert_state {
    ($state:expr, $expected:pat) => {
        debug_assert!(
            matches!($state, $expected),
            "unexpected state: {:?}",
            $state
        )
    };
}

impl State {
    pub fn have_token(&self) -> bool {
        match self {
            State::Offline { .. }
            | State::PassiveIdle { .. }
            | State::ListenToken { .. }
            | State::ActiveIdle { .. }
            | State::PassToken { .. }
            | State::CheckTokenPass { .. } => false,
            State::ClaimToken { .. }
            | State::UseToken { .. }
            | State::AwaitDataResponse { .. }
            | State::AwaitStatusResponse { .. } => true,
        }
    }

    fn transition_offline(&mut self) {
        debug_assert_state!(
            self,
            State::Offline { .. }
                | State::PassiveIdle { .. }
                | State::ListenToken { .. }
                | State::PassToken { .. }
        );
        *self = State::Offline
    }

    fn transition_passive_idle(&mut self) {
        debug_assert_state!(self, State::Offline { .. } | State::PassiveIdle { .. });
        *self = State::PassiveIdle;
    }

    fn transition_listen_token(&mut self) {
        debug_assert_state!(
            self,
            State::ListenToken { .. } | State::Offline { .. } | State::ActiveIdle { .. }
        );
        *self = State::ListenToken {
            status_request: None,
            collision_count: 0,
        };
    }

    fn transition_active_idle(&mut self) {
        debug_assert_state!(
            self,
            State::ActiveIdle { .. }
                | State::ListenToken { .. }
                | State::UseToken { .. }
                | State::AwaitDataResponse { .. }
                | State::CheckTokenPass { .. }
                | State::AwaitStatusResponse { .. }
        );
        *self = State::ActiveIdle {
            status_request: None,
        };
    }

    fn transition_use_token(&mut self) {
        debug_assert_state!(
            self,
            State::UseToken { .. }
                | State::ClaimToken { .. }
                | State::PassToken { .. }
                | State::AwaitDataResponse { .. }
                | State::ActiveIdle { .. }
        );
        *self = State::UseToken;
    }

    fn transition_claim_token(&mut self) {
        debug_assert_state!(
            self,
            State::ClaimToken { .. } | State::ListenToken { .. } | State::ActiveIdle { .. }
        );
        *self = State::ClaimToken { first: true };
    }

    fn transition_await_data_response(&mut self) {
        debug_assert_state!(
            self,
            State::AwaitDataResponse { .. } | State::UseToken { .. }
        );
        *self = State::AwaitDataResponse;
    }

    fn transition_pass_token(&mut self, do_gap: bool) {
        debug_assert_state!(
            self,
            State::PassToken { .. }
                | State::UseToken { .. }
                | State::ClaimToken { .. }
                | State::CheckTokenPass { .. }
                | State::AwaitStatusResponse { .. }
        );
        *self = State::PassToken { do_gap };
    }

    fn transition_check_token_pass(&mut self) {
        debug_assert_state!(self, State::CheckTokenPass { .. } | State::PassToken { .. });
        *self = State::CheckTokenPass;
    }

    fn transition_await_status_response(&mut self, address: crate::Address) {
        debug_assert_state!(
            self,
            State::AwaitStatusResponse { .. } | State::PassToken { .. }
        );
        *self = State::AwaitStatusResponse { address };
    }
}

#[derive(Debug)]
pub struct FdlActiveStation {
    /// Parameters for the connected bus and this station
    p: crate::fdl::Parameters,

    /// Management of the token ring
    token_ring: crate::fdl::TokenRing,

    /// Connectivity status of this station
    connectivity_state: ConnectivityState,

    // State of GAP polling
    gap_state: GapState,

    /// State of the active station
    state: State,

    /// Timestamp of the last time we found the bus to be active (= someone transmitting)
    last_bus_activity: Option<crate::time::Instant>,

    /// Amount of bytes pending in the receive buffer.
    ///
    /// This known value is compared to the latest one reported by the PHY to find out whether new
    /// data was received since the last poll.
    pending_bytes: usize,
}

impl FdlActiveStation {
    pub fn new(param: crate::fdl::Parameters) -> Self {
        param.debug_assert_consistency();

        Self {
            token_ring: crate::fdl::TokenRing::new(&param),
            // A station must always start offline
            connectivity_state: ConnectivityState::Offline,
            gap_state: GapState::NextPoll {
                next_addr: param.address.wrapping_add(1),
            },
            state: State::Offline,
            last_bus_activity: None,
            pending_bytes: 0,
            p: param,
        }
    }

    /// Return a reference to the parameters configured for this FDL active station.
    #[inline(always)]
    pub fn parameters(&self) -> &crate::fdl::Parameters {
        &self.p
    }

    #[inline(always)]
    pub fn connectivity_state(&self) -> ConnectivityState {
        self.connectivity_state
    }

    #[inline]
    pub fn set_state(&mut self, state: ConnectivityState) {
        log::info!("FDL active station entering state \"{:?}\"", state);
        self.connectivity_state = state;

        if state == ConnectivityState::Offline {
            // If we are going offline, reset all internal state by recreating the FDL station.
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
    events: E,
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

    pub fn with_events<E>(self, events: E) -> PollResult<E> {
        PollResult { events }
    }
}

impl<E: Default> From<PollDone> for PollResult<E> {
    fn from(value: PollDone) -> Self {
        PollResult {
            events: Default::default(),
        }
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

impl FdlActiveStation {
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
        let phy_transmitting = phy.poll_transmission(now);
        if phy_transmitting || self.last_bus_activity.map(|l| now <= l).unwrap_or(false) {
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
        if now <= (*self.last_bus_activity.get_or_insert(now) + self.p.bits_to_time(33)) {
            Some(PollDone::waiting_for_delay())
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
        PollDone::waiting_for_transmission()
    }

    fn check_for_bus_activity(&mut self, now: crate::time::Instant, phy: &mut impl ProfibusPhy) {
        let pending_bytes = phy.poll_pending_received_bytes(now);
        if pending_bytes > self.pending_bytes {
            self.mark_bus_activity(now);
            self.pending_bytes = pending_bytes;
        }
    }

    /// Mark receival of a telegram.
    fn mark_rx(&mut self, now: crate::time::Instant) {
        self.pending_bytes = 0;
        self.mark_bus_activity(now);
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
}

impl FdlActiveStation {
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
            if self.token_ring.ready_for_ring() {
                log::warn!("Token lost! Generating a new one.");
            } else {
                log::info!("Generating new token due to silent bus.");
            }

            self.state.transition_claim_token();
            Some(self.do_claim_token(now, phy))
        } else {
            None
        }
    }

    fn next_gap_poll(&self, addr: u8) -> GapState {
        let next_station = self.token_ring.next_station();
        if addr >= next_station && next_station > self.p.address {
            // Don't poll beyond the gap.
            GapState::Waiting { rotation_count: 0 }
        } else if (addr + 1) == self.p.address {
            // Don't poll self.
            GapState::Waiting { rotation_count: 0 }
        } else if addr == (self.p.highest_station_address - 1) {
            // Wrap around.
            GapState::NextPoll { next_addr: 0 }
        } else {
            GapState::NextPoll {
                next_addr: addr + 1,
            }
        }
    }
}

/// State Machine of the FDL active station
impl FdlActiveStation {
    #[must_use = "poll done marker"]
    fn do_listen_token<'a, PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) -> PollDone {
        debug_assert_state!(self.state, State::ListenToken { .. });

        return_if_done!(self.handle_lost_token(now, phy));

        // Handle pending response to a telegram request we received
        if let State::ListenToken {
            status_request: Some(status_request_source),
            collision_count,
        } = self.state
        {
            return_if_done!(self.wait_synchronization_pause(now));

            // We must only respond to be ready (=without token) when the request is sent by our
            // known previous neighbor station.
            let state = if self.token_ring.ready_for_ring()
                && status_request_source == self.token_ring.previous_station()
            {
                crate::fdl::ResponseState::MasterWithoutToken
            } else {
                crate::fdl::ResponseState::MasterNotReady
            };

            let tx_res = phy
                .transmit_telegram(now, |tx| {
                    Some(tx.send_fdl_status_response(
                        status_request_source,
                        self.p.address,
                        state,
                        crate::fdl::ResponseStatus::Ok,
                    ))
                })
                .unwrap();

            if self.token_ring.ready_for_ring() {
                self.state.transition_active_idle();
            } else {
                self.state = State::ListenToken {
                    status_request: None,
                    collision_count,
                };
            }
            return self.mark_tx(now, tx_res.bytes_sent());
        }

        // Handle received telegrams
        phy.receive_telegram(now, |telegram| {
            self.mark_rx(now);

            // Handle address collision detection
            if telegram.source_address() == Some(self.p.address) {
                let State::ListenToken { collision_count, .. } = &mut self.state else {
                    unreachable!()
                };

                *collision_count += 1;

                match *collision_count {
                    1 => {
                        log::warn!("Witnessed collision of another active station with own address (#{})!", self.p.address);
                    }
                    2 | _ => {
                        log::warn!(
                            "Witnessed second collision of another active station with own address (#{}), going offline.",
                            self.p.address,
                        );
                        self.set_offline();
                    }
                }
                return PollDone::waiting_for_bus();
            }

            match telegram {
                // Handle witnessing a token telegram
                crate::fdl::Telegram::Token(token_telegram) => {
                    self.token_ring.witness_token_pass(token_telegram.sa, token_telegram.da);
                    PollDone::waiting_for_bus()
                }

                // Handle FDL requests sent to us
                crate::fdl::Telegram::Data(data_telegram)
                    if data_telegram.is_fdl_status_request().is_some()
                        && data_telegram.h.da == self.p.address =>
                {

                    let State::ListenToken { status_request, .. } = &mut self.state else {
                        unreachable!()
                    };
                    *status_request = Some(data_telegram.h.sa);
                    PollDone::waiting_for_delay()
                }
                _ => PollDone::waiting_for_bus(),
            }
        }).unwrap_or(PollDone::waiting_for_bus())
    }

    #[must_use = "poll done marker"]
    fn do_active_idle<'a, PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) -> PollDone {
        debug_assert_state!(self.state, State::ActiveIdle { .. });

        return_if_done!(self.handle_lost_token(now, phy));

        // Handle pending response to a telegram request we received
        if let State::ActiveIdle {
            status_request: Some(status_request_source),
            ..
        } = self.state
        {
            return_if_done!(self.wait_synchronization_pause(now));

            let tx_res = phy
                .transmit_telegram(now, |tx| {
                    Some(tx.send_fdl_status_response(
                        status_request_source,
                        self.p.address,
                        crate::fdl::ResponseState::MasterInRing,
                        crate::fdl::ResponseStatus::Ok,
                    ))
                })
                .unwrap();

            let State::ActiveIdle { status_request, .. } = &mut self.state else {
                unreachable!()
            };
            *status_request = None;

            return self.mark_tx(now, tx_res.bytes_sent());
        }

        phy.receive_telegram(now, |telegram| {
            self.mark_rx(now);

            match telegram {
                // Handle any token telegrams
                crate::fdl::Telegram::Token(token_telegram) => {
                    self.token_ring
                        .witness_token_pass(token_telegram.sa, token_telegram.da);

                    if token_telegram.da != self.p.address {
                        PollDone::waiting_for_bus()
                    } else {
                        // We may only accept the token from the known neighbor (on their first try)
                        if token_telegram.sa == self.token_ring.previous_station() {
                            self.state.transition_use_token();
                            PollDone::waiting_for_delay()
                        } else {
                            log::warn!("TODO: Handle token from unknown neighbor");
                            PollDone::waiting_for_bus()
                        }
                    }
                }

                // Handle FDL requests sent to us
                crate::fdl::Telegram::Data(data_telegram)
                    if data_telegram.is_fdl_status_request().is_some()
                        && data_telegram.h.da == self.p.address =>
                {
                    let State::ActiveIdle { status_request, .. } = &mut self.state else {
                        unreachable!()
                    };
                    *status_request = Some(data_telegram.h.sa);
                    PollDone::waiting_for_delay()
                }
                _ => PollDone::waiting_for_bus(),
            }
        })
        .unwrap_or(PollDone::waiting_for_bus())
    }

    #[must_use = "poll done marker"]
    fn do_claim_token<'a, PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) -> PollDone {
        debug_assert_state!(self.state, State::ClaimToken { .. });

        // The token is claimed by sending a telegram to ourselves twice.
        return_if_done!(self.wait_synchronization_pause(now));
        let tx_res = phy
            .transmit_telegram(now, |tx| {
                Some(tx.send_token_telegram(self.p.address, self.p.address))
            })
            .unwrap();

        match self.state {
            State::ClaimToken { first: true } => {
                // This will lead to sending the claim token telegram again
                self.state = State::ClaimToken { first: false };
            }
            State::ClaimToken { first: false } => {
                // Now we have claimed the token and can proceed to use it.
                self.state.transition_use_token();
            }
            _ => unreachable!(),
        }

        self.mark_tx(now, tx_res.bytes_sent())
    }

    #[must_use = "poll done marker"]
    fn do_use_token<'a, PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) -> PollDone {
        debug_assert_state!(self.state, State::UseToken);

        // TODO: Rotation timer
        // TODO: Message exchange cycles

        self.state.transition_pass_token(true);
        PollDone::waiting_for_delay()
    }

    #[must_use = "poll done marker"]
    fn do_pass_token<'a, PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) -> PollDone {
        debug_assert_state!(self.state, State::PassToken { .. });

        return_if_done!(self.wait_synchronization_pause(now));

        if let State::PassToken { do_gap: true } = self.state {
            if let GapState::Waiting { rotation_count } = self.gap_state {
                if rotation_count > self.p.gap_wait_rotations {
                    // We're done waiting, do a poll now!
                    log::debug!("Starting next gap polling cycle!");
                    self.gap_state = self.next_gap_poll(self.p.address);
                }
            }

            if let GapState::NextPoll { next_addr } = self.gap_state {
                self.gap_state = self.next_gap_poll(next_addr);

                let tx_res = phy
                    .transmit_telegram(now, |tx| {
                        Some(tx.send_fdl_status_request(next_addr, self.p.address))
                    })
                    .unwrap();

                self.state.transition_await_status_response(next_addr);

                return self.mark_tx(now, tx_res.bytes_sent());
            }
        }

        let tx_res = phy
            .transmit_telegram(now, |tx| {
                Some(tx.send_token_telegram(self.token_ring.next_station(), self.p.address))
            })
            .unwrap();

        if self.token_ring.next_station() == self.p.address {
            self.state.transition_use_token();
        } else {
            self.state.transition_check_token_pass();
        }

        self.mark_tx(now, tx_res.bytes_sent())
    }

    #[must_use = "poll done marker"]
    fn do_await_status_response<'a, PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) -> PollDone {
        debug_assert_state!(self.state, State::AwaitStatusResponse { .. });

        let State::AwaitStatusResponse { address, .. } = self.state else {
            unreachable!()
        };

        let received = phy.receive_telegram(now, |telegram| {
            self.mark_rx(now);

            if let crate::fdl::Telegram::Data(telegram) = &telegram {
                if telegram.h.sa == address && telegram.h.da == self.p.address {
                    if let crate::fdl::FunctionCode::Response { state, status } = telegram.h.fc {
                        log::trace!("Address #{address} responded");
                        if status == crate::fdl::ResponseStatus::Ok
                            && matches!(state, crate::fdl::ResponseState::MasterWithoutToken | crate::fdl::ResponseState::MasterInRing) {
                            self.token_ring.set_next_station(address);
                        }
                        self.state.transition_pass_token(false);
                        return PollDone::waiting_for_delay();
                    }
                }

            }

            log::warn!("Received unexpected telegram while waiting for status reply from #{address}: {telegram:?}");
            self.state.transition_active_idle();
            PollDone::waiting_for_bus()
        });

        if let Some(res) = received {
            return res;
        }

        if self.check_slot_expired(now) {
            log::trace!("No reply from #{address}");
            self.state.transition_pass_token(false);
            PollDone::waiting_for_delay()
        } else {
            PollDone::waiting_for_bus()
        }
    }

    #[must_use = "poll done marker"]
    fn do_check_token_pass<'a, PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) -> PollDone {
        debug_assert_state!(self.state, State::CheckTokenPass);

        if self.check_slot_expired(now) {
            log::warn!(
                "Token was apparently not received by #{}, resending...",
                self.token_ring.next_station()
            );
            self.state.transition_pass_token(false);
            return PollDone::waiting_for_delay();
        }

        phy.receive_telegram(now, |telegram| {
            self.mark_rx(now);

            if telegram.source_address() != Some(self.token_ring.next_station()) {
                log::warn!(
                    "Unexpected station #{} transmitting after token pass to #{}",
                    telegram.source_address().unwrap(),
                    self.token_ring.next_station()
                );
            }

            // TODO: Handle telegrams directed at us (token or status request)

            self.state.transition_active_idle();
        });

        PollDone::waiting_for_bus()
    }

    pub fn poll<'a, PHY: ProfibusPhy, APP: FdlApplication>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
        app: &mut APP,
    ) -> APP::Events {
        let result = self.poll_inner(now, phy, app);
        result.events
    }

    fn poll_inner<'a, PHY: ProfibusPhy, APP: FdlApplication>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
        app: &mut APP,
    ) -> PollResult<APP::Events> {
        // Handle connectivity_state changes
        match self.connectivity_state {
            ConnectivityState::Offline => {
                debug_assert!(matches!(self.state, State::Offline));
                // When we are offline, don't do anything at all.
                return PollDone::offline().into();
            }
            ConnectivityState::Passive => {
                // TODO: Check if these are all the states from which we can transition to passive
                // idle
                match &self.state {
                    State::ActiveIdle { .. } | State::ListenToken { .. } | State::Offline => {
                        self.state.transition_passive_idle();
                    }
                    State::PassiveIdle => (),
                    s => {
                        log::debug!("Can't transition from \"{s:?}\" to PassiveIdle");
                    }
                }
            }
            ConnectivityState::Online => {
                if matches!(self.state, State::Offline | State::PassiveIdle) {
                    self.state.transition_listen_token();
                }
            }
        }

        // When a transmission is ongoing, we cannot do anything else in the meantime.  Thus,
        // return immediately in this case.
        return_if_done!(self.check_for_ongoing_transmision(now, phy));

        match &self.state {
            State::Offline { .. } => unreachable!(),
            State::ListenToken { .. } => self.do_listen_token(now, phy).into(),
            State::ClaimToken { .. } => self.do_claim_token(now, phy).into(),
            State::UseToken { .. } => self.do_use_token(now, phy).into(),
            State::PassToken { .. } => self.do_pass_token(now, phy).into(),
            State::CheckTokenPass { .. } => self.do_check_token_pass(now, phy).into(),
            State::ActiveIdle { .. } => self.do_active_idle(now, phy).into(),
            State::AwaitStatusResponse { .. } => self.do_await_status_response(now, phy).into(),
            s => todo!("Active station state {s:?} not implemented yet!"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ensure the `FdlActiveStation` struct size doesn't completely get out of control.
    #[test]
    fn fdl_active_station_struct_size() {
        let size = std::mem::size_of::<FdlActiveStation>();
        println!("FDL active station struct is {size} bytes large.");
        assert!(size <= 256);
    }

    #[test]
    fn fdl_active_station_smoke() {
        crate::test_utils::prepare_test_logger();

        let mut phy = crate::phy::SimulatorPhy::new(crate::Baudrate::B19200, "phy");
        let mut fdl = FdlActiveStation::new(Default::default());

        crate::test_utils::set_active_addr(fdl.parameters().address);

        fdl.set_online();
        fdl.set_offline();
        fdl.set_online();

        let mut now = crate::time::Instant::ZERO;
        while now.total_millis() < 200 {
            fdl.poll(now, &mut phy, &mut ());

            now += crate::time::Duration::from_micros(100);
            phy.set_bus_time(now);
            crate::test_utils::set_log_timestamp(now);
        }
    }
}
