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
    /// Planned token circulation time
    pub ttr: u32,
    /// GAP: update factor (how many token rotations to wait before polling the gap again)
    pub gap: u8,
    /// HSA: Highest projected station address
    pub hsa: u8,
    /// Maximum number of retries when no answer was received
    pub max_retry_limit: u8,
}

impl Default for Parameters {
    fn default() -> Self {
        Parameters {
            address: 1,
            baudrate: crate::fdl::Baudrate::B19200,
            slot_bits: 100,
            ttr: 20000, // TODO: really sane default?  This was at least recommended somewhere...
            gap: 10,    // TODO: sane default?
            hsa: 125,
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
}

#[derive(Debug)]
enum GapState {
    Waiting(u8),
    NextCheck(u8),
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

    gap_state: GapState,

    /// List of live stations.
    live_list: bitvec::BitArr!(for 256),
}

impl FdlMaster {
    pub fn new(param: Parameters) -> Self {
        let mut live_list = bitvec::array::BitArray::ZERO;
        // Mark ourselves as "live".
        live_list.set(param.address as usize, true);

        Self {
            next_master: param.address,
            last_telegram_time: None,
            have_token: false,
            last_token_time: None,
            gap_state: GapState::NextCheck(param.address.wrapping_add(1)),
            live_list,

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
}

impl FdlMaster {
    fn acquire_token(&mut self, now: crate::time::Instant, token: &crate::fdl::TokenTelegram) {
        debug_assert!(token.da == self.p.address);
        self.have_token = true;
        self.last_token_time = Some(now);
    }

    #[must_use = "tx token"]
    fn forward_token(&mut self, now: crate::time::Instant, phy: &mut impl ProfibusPhy) -> TxMarker {
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
    fn handle_with_token(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
    ) -> Option<TxMarker> {
        debug_assert!(self.have_token);
        let acquire_time = *self.last_token_time.get_or_insert(now);
        if (now - acquire_time) >= self.p.slot_time() * 6 {
            // TODO: For now, just immediately send the token to the next master after one slot time.
            Some(self.forward_token(now, phy))
        } else {
            None
        }
    }

    #[must_use = "tx token"]
    fn handle_without_token(
        &mut self,
        now: crate::time::Instant,
        phy: &mut impl ProfibusPhy,
    ) -> Option<TxMarker> {
        let maybe_received = phy.receive_telegram();
        if maybe_received.is_some() {
            self.last_telegram_time = Some(now);
        }
        match maybe_received {
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
