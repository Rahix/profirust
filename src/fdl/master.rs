#![deny(unused_must_use)]
use crate::phy::ProfibusPhy;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Parameters {
    /// Station address for this master
    pub address: u8,
    /// Baudrate
    pub baudrate: crate::fdl::Baudrate,
    /// T<sub>SL</sub>: Slot time in bits
    pub slot_bits: u16,
    /// Time until the token should have rotated through all masters once.
    pub token_rotation_bits: u32,
    /// GAP: update factor (how many token rotations to wait before polling the gap again)
    pub gap_wait_rotations: u8,
    /// HSA: Highest projected station address
    pub highest_station_address: u8,
    /// Maximum number of retries when no answer was received
    pub max_retry_limit: u8,
}

impl Default for Parameters {
    fn default() -> Self {
        Parameters {
            address: 1,
            baudrate: crate::fdl::Baudrate::B19200,
            slot_bits: 100,
            token_rotation_bits: 20000, // TODO: really sane default?  This was at least recommended somewhere...
            gap_wait_rotations: 100,    // TODO: sane default?
            highest_station_address: 125,
            max_retry_limit: 6, // TODO: sane default?
        }
    }
}

impl Parameters {
    pub fn bits_to_time(&self, bits: u32) -> crate::time::Duration {
        self.baudrate.bits_to_time(bits)
    }

    /// T<sub>SL</sub> (slit time) converted to duration
    pub fn slot_time(&self) -> crate::time::Duration {
        self.bits_to_time(self.slot_bits as u32)
    }

    /// Timeout after which the token is considered lost.
    ///
    /// Calculated as 6 * T<sub>SL</sub> + 2 * Addr * T<sub>SL</sub>.
    pub fn token_lost_timeout(&self) -> crate::time::Duration {
        let timeout_bits =
            6 * self.slot_bits as u32 + 2 * self.address as u32 * self.slot_bits as u32;
        self.bits_to_time(timeout_bits)
    }

    /// T<sub>TR</sub> (projected token rotation time)
    pub fn token_rotation_time(&self) -> crate::time::Duration {
        self.bits_to_time(self.token_rotation_bits)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum GapState {
    /// Waiting for some time until the next gap polling cycle is performed.
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum MasterState {
    /// Master has nothing to do.
    Idle,

    /// Awaiting a response telegram from a station with the given address.
    AwaitingResponse(u8, crate::time::Instant),

    /// Awaiting response to an FDL status telegram from the gap polling machinery.
    AwaitingGapResponse(u8, crate::time::Instant),
}

#[derive(Debug)]
pub struct FdlMaster {
    p: Parameters,

    /// Address of the next master in the token ring.
    ///
    /// This value is always valid and will be our own station address when no other master is
    /// known.
    next_master: u8,

    /// Timestamp of the last telegram on the bus.
    ///
    /// Used for detecting timeouts.
    last_telegram_time: Option<crate::time::Instant>,

    /// Whether we currently hold the token.
    have_token: bool,

    /// Timestamp of last token acquisition.
    last_token_time: Option<crate::time::Instant>,
    /// Timestamp of the second to last token acquisition.
    previous_token_time: Option<crate::time::Instant>,

    /// State of the gap polling machinery.
    gap_state: GapState,

    /// List of live stations.
    live_list: bitvec::BitArr!(for 256),

    /// State of the master.
    master_state: MasterState,
}

impl FdlMaster {
    pub fn new(param: Parameters) -> Self {
        let mut live_list = bitvec::array::BitArray::ZERO;
        // Mark ourselves as "live".
        live_list.set(param.address as usize, true);

        debug_assert!(param.highest_station_address <= 125);

        Self {
            next_master: param.address,
            last_telegram_time: None,
            have_token: false,
            last_token_time: None,
            previous_token_time: None,
            gap_state: GapState::NextPoll(param.address.wrapping_add(1)),
            live_list,
            master_state: MasterState::Idle,

            p: param,
        }
    }

    pub fn parameters(&self) -> &Parameters {
        &self.p
    }
}

#[must_use = "Transmission marker must lead to exit of poll function!"]
struct TxMarker();

macro_rules! return_if_tx {
    ($expr:expr) => {
        match $expr {
            e @ Some(TxMarker()) => return e,
            None => (),
        }
    };
}

impl FdlMaster {
    fn check_for_ongoing_transmision(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
    ) -> Option<TxMarker> {
        if phy.is_transmitting() {
            self.last_telegram_time = Some(now);
            Some(TxMarker())
        } else {
            None
        }
    }

    fn transmit_telegram<'a, T: Into<crate::fdl::Telegram<'a>>>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
        telegram: T,
    ) -> TxMarker {
        phy.transmit_telegram(telegram.into());
        self.last_telegram_time = Some(now);
        TxMarker()
    }

    fn try_receive_telegram<'a>(
        &mut self,
        now: crate::time::Instant,
        phy: &'a mut impl ProfibusPhy,
    ) -> Option<crate::fdl::Telegram<'a>> {
        let maybe_received = phy.receive_telegram();
        if maybe_received.is_some() {
            self.last_telegram_time = Some(now);
        }
        maybe_received
    }
}

impl FdlMaster {
    fn acquire_token(&mut self, now: crate::time::Instant, token: &crate::fdl::TokenTelegram) {
        debug_assert!(token.da == self.p.address);
        self.have_token = true;
        self.previous_token_time = self.last_token_time;
        self.last_token_time = Some(now);
        self.gap_state.increment_wait();
    }

    #[must_use = "tx token"]
    fn forward_token(&mut self, now: crate::time::Instant, phy: &mut impl ProfibusPhy) -> TxMarker {
        self.master_state = MasterState::Idle;
        self.have_token = false;
        let token_telegram = crate::fdl::TokenTelegram::new(self.next_master, self.p.address);
        if self.next_master == self.p.address {
            // Special case when the token is also fowarded to ourselves.
            self.acquire_token(now, &token_telegram);
        }
        self.transmit_telegram(now, phy, token_telegram)
    }

    #[must_use = "tx token"]
    fn handle_lost_token(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
    ) -> Option<TxMarker> {
        let last_telegram_time = *self.last_telegram_time.get_or_insert(now);
        if (now - last_telegram_time) >= self.p.token_lost_timeout() {
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
        high_prio_only: bool,
    ) -> Option<TxMarker> {
        None
    }

    fn check_for_response(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
    ) -> bool {
        todo!()
    }

    fn check_for_status_response(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
        addr: u8,
    ) -> bool {
        if let Some(crate::fdl::Telegram::Data(telegram)) = self.try_receive_telegram(now, phy) {
            if telegram.sa != addr {
                log::warn!("Unexpected telegram? {:?}", telegram);
                return false;
            }

            if let crate::fdl::FunctionCode::Response { state, status } = telegram.fc {
                if state == crate::fdl::ResponseState::MasterWithoutToken
                    || state == crate::fdl::ResponseState::MasterInRing
                {
                    self.next_master = telegram.sa;
                }
                return true;
            }

            log::warn!("Unexpected telegram? {:?}", telegram);
            return false;
        }

        return false;
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
    ) -> Option<TxMarker> {
        debug_assert!(matches!(self.master_state, MasterState::Idle));

        if let GapState::Waiting(r) = self.gap_state {
            if r >= self.p.gap_wait_rotations {
                // We're done waiting, do a poll now!
                log::debug!("Starting next gap polling cycle!");
                self.gap_state = self.next_gap_poll(self.p.address);
            }
        }

        if let GapState::NextPoll(addr) = self.gap_state {
            self.gap_state = self.next_gap_poll(addr);
            self.master_state = MasterState::AwaitingGapResponse(addr, now);

            let status_request =
                crate::fdl::DataTelegram::new_fdl_status_request(addr, self.p.address);
            return Some(self.transmit_telegram(now, phy, status_request));
        }

        None
    }

    #[must_use = "tx token"]
    fn handle_with_token(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
    ) -> Option<TxMarker> {
        debug_assert!(self.have_token);

        // First check for ongoing message cycles and handle them.
        match self.master_state {
            MasterState::Idle => (),
            MasterState::AwaitingResponse(addr, sent_time) => {
                if self.check_for_response(now, phy) {
                    self.master_state = MasterState::Idle;
                } else if (now - sent_time) >= self.p.slot_time() {
                    todo!("handle message cycle response timeout");
                } else {
                    // Still waiting for the response, nothing to do here.
                    // TODO: shouldn't we also return the marker here?
                    return None;
                }
            }
            MasterState::AwaitingGapResponse(addr, sent_time) => {
                if self.check_for_status_response(now, phy, addr) {
                    // After the gap response, we pass on the token.
                    self.live_list.set(addr as usize, true);
                    return Some(self.forward_token(now, phy));
                } else if (now - sent_time) >= self.p.slot_time() {
                    // Mark this address as not alive and pass on the token.
                    self.live_list.set(addr as usize, false);
                    return Some(self.forward_token(now, phy));
                } else {
                    // Still waiting for the response, nothing to do here.
                    // TODO: shouldn't we also return the marker here?
                    return None;
                }
            }
        }

        // Check if there is still time to start a message cycle.
        if let Some(rotation_time) = self.previous_token_time.map(|p| now - p) {
            if rotation_time >= self.p.token_rotation_time() {
                // If we're over the rotation time and just acquired the token, we are allowed to
                // perform one more high priority message cycle.
                if self.last_token_time == Some(now) {
                    return_if_tx!(self.try_start_message_cycle(now, phy, true));
                }

                // In any other case, we pass on the token to the next master.
                return Some(self.forward_token(now, phy));
            }
        }

        // We have time, try doing useful things.
        return_if_tx!(self.try_start_message_cycle(now, phy, false));

        // If we end up here, there's nothing useful left to do so now handle the gap polling cycle.
        return_if_tx!(self.handle_gap(now, phy));

        // And if even the gap poll didn't lead to a message, pass token immediately.
        return Some(self.forward_token(now, phy));
    }

    #[must_use = "tx token"]
    fn handle_without_token(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
    ) -> Option<TxMarker> {
        match self.try_receive_telegram(now, phy) {
            Some(crate::fdl::Telegram::Token(token_telegram)) => {
                if token_telegram.da == self.p.address {
                    // Heyy, we got the token!
                    self.acquire_token(now, &token_telegram);
                    return None;
                } else {
                    log::trace!(
                        "Witnessed token passing: {} => {}",
                        token_telegram.sa,
                        token_telegram.da,
                    );
                    return None;
                }
            }
            // Telegram for us!
            Some(crate::fdl::Telegram::Data(telegram)) if telegram.da == self.p.address => {
                if let Some(da) = telegram.is_fdl_status_request() {
                    let response = crate::fdl::DataTelegram::new_fdl_status_response(
                        da,
                        self.p.address,
                        crate::fdl::ResponseState::MasterWithoutToken,
                        crate::fdl::ResponseStatus::Ok,
                    );

                    return Some(self.transmit_telegram(now, phy, response));
                }
            }
            // TODO: We must at least respond to FDL Status requests so we may at some point get
            // into the ring if other masters are present.
            Some(t) => log::trace!("Unhandled telegram: {:?}", t),
            None => (),
        };
        None
    }

    pub fn poll<PHY: ProfibusPhy>(&mut self, now: crate::time::Instant, phy: &mut PHY) {
        let _ = self.poll_inner(now, phy);
    }

    fn poll_inner<PHY: ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) -> Option<TxMarker> {
        return_if_tx!(self.check_for_ongoing_transmision(now, phy));

        if self.have_token {
            log::trace!("{} has token!", self.p.address);
            return_if_tx!(self.handle_with_token(now, phy));
        } else {
            return_if_tx!(self.handle_without_token(now, phy));
            // We may have just received the token so do one more pass "with token".
            if self.have_token {
                return_if_tx!(self.handle_with_token(now, phy));
            }
        }

        return_if_tx!(self.handle_lost_token(now, phy));
        None
    }
}
