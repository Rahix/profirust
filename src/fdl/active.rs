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

#[derive(Debug, PartialEq, Eq)]
enum State {
    Offline,
    PassiveIdle,
    ListenToken,
    ActiveIdle,
    UseToken,
    ClaimToken { first: bool },
    AwaitDataResponse,
    PassToken,
    CheckTokenPass,
    AwaitStatusResponse,
}

impl State {
    pub fn have_token(&self) -> bool {
        match self {
            State::Offline
            | State::PassiveIdle
            | State::ListenToken
            | State::ActiveIdle
            | State::PassToken
            | State::CheckTokenPass => false,
            State::ClaimToken { .. }
            | State::UseToken
            | State::AwaitDataResponse
            | State::AwaitStatusResponse => true,
        }
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

            self.state = State::ClaimToken { first: true };
            Some(self.do_claim_token(now, phy))
        } else {
            None
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
        debug_assert_eq!(self.state, State::ListenToken);

        return_if_done!(self.handle_lost_token(now, phy));

        // TODO: Respond to status requests
        // TODO: Fill LAS
        // TODO: Detect address collision
        phy.receive_telegram(now, |telegram| {
            self.mark_rx(now);

            todo!("Need to implement ListenToken state handler")
        });

        PollDone::waiting_for_bus()
    }

    #[must_use = "poll done marker"]
    fn do_active_idle<'a, PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) -> PollDone {
        debug_assert_eq!(self.state, State::ActiveIdle);

        // Check for token lost timeout
        todo!("do_active_idle")
    }

    #[must_use = "poll done marker"]
    fn do_claim_token<'a, PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) -> PollDone {
        debug_assert!(
            matches!(self.state, State::ClaimToken { .. }),
            "Wrong state for do_claim_token: {:?}",
            self.state
        );

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
                self.state = State::UseToken;
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
        debug_assert!(
            matches!(self.state, State::UseToken),
            "Wrong state for do_use_token: {:?}",
            self.state
        );

        // TODO: Rotation timer
        // TODO: Message exchange cycles

        self.state = State::PassToken;
        PollDone::waiting_for_delay()
    }

    #[must_use = "poll done marker"]
    fn do_pass_token<'a, PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) -> PollDone {
        debug_assert!(
            matches!(self.state, State::PassToken),
            "Wrong state for do_pass_token: {:?}",
            self.state
        );

        // TODO: GAPL update

        return_if_done!(self.wait_synchronization_pause(now));
        let tx_res = phy
            .transmit_telegram(now, |tx| {
                Some(tx.send_token_telegram(self.token_ring.next_station(), self.p.address))
            })
            .unwrap();

        if self.token_ring.next_station() == self.p.address {
            self.state = State::UseToken;
        } else {
            self.state = State::CheckTokenPass;
        }

        self.mark_tx(now, tx_res.bytes_sent())
    }

    #[must_use = "poll done marker"]
    fn do_check_token_pass<'a, PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) -> PollDone {
        debug_assert!(
            matches!(self.state, State::CheckTokenPass),
            "Wrong state for do_check_token_pass: {:?}",
            self.state
        );

        // TODO: Actually check the token pass
        log::trace!("Ignoring whether the token was received (TODO)!");

        self.state = State::ActiveIdle;
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
                    State::ActiveIdle | State::ListenToken | State::Offline => {
                        self.state = State::PassiveIdle;
                    }
                    State::PassiveIdle => (),
                    s => {
                        log::debug!("Can't transition from \"{s:?}\" to PassiveIdle");
                    }
                }
            }
            ConnectivityState::Online => {
                if matches!(self.state, State::Offline | State::PassiveIdle) {
                    self.state = State::ListenToken;
                }
            }
        }

        // When a transmission is ongoing, we cannot do anything else in the meantime.  Thus,
        // return immediately in this case.
        return_if_done!(self.check_for_ongoing_transmision(now, phy));

        match &self.state {
            State::Offline => unreachable!(),
            State::ListenToken => self.do_listen_token(now, phy).into(),
            State::ClaimToken { .. } => self.do_claim_token(now, phy).into(),
            State::UseToken => self.do_use_token(now, phy).into(),
            State::PassToken => self.do_pass_token(now, phy).into(),
            State::CheckTokenPass => self.do_check_token_pass(now, phy).into(),
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
