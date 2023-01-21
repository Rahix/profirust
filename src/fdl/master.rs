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
    ///
    /// If we have the token, `have_token` stores the timestamp when we got it.  This is used to
    /// check whether we still have time to perform more transmissions.
    have_token: Option<crate::time::Instant>,
}

impl FdlMaster {
    pub fn new(param: Parameters) -> Self {
        Self {
            next_master: param.address,
            p: param,
            last_telegram_time: None,
            have_token: None,
        }
    }

    pub fn parameters(&self) -> &Parameters {
        &self.p
    }

    fn handle_lost_token<PHY: crate::phy::ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) -> bool {
        let last_telegram_time = *self.last_telegram_time.get_or_insert(now);
        if (now - last_telegram_time) >= self.p.token_lost_timeout() {
            log::warn!("Token lost! Generating a new one.");
            self.next_master = self.p.address;
            let token_telegram = crate::fdl::TokenTelegram::new(self.next_master, self.p.address);
            // We are allowed to send here, even though we don't currently hold the token.
            phy.transmit_telegram(token_telegram.into());
            self.last_telegram_time = Some(now);
            self.have_token = Some(now);
            true
        } else {
            false
        }
    }

    fn handle_with_token<PHY: crate::phy::ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) {
        let acquire_time = *self.have_token.as_ref().unwrap();
        if (now - acquire_time) >= self.p.slot_time() * 6 {
            // TODO: For now, just immediately send the token to the next master after one slot time.
            let token_telegram = crate::fdl::TokenTelegram::new(self.next_master, self.p.address);
            phy.transmit_telegram(token_telegram.into());
            self.last_telegram_time = Some(now);
            self.have_token = if self.next_master == self.p.address {
                Some(now)
            } else {
                None
            };
        }
    }

    fn handle_without_token<PHY: crate::phy::ProfibusPhy>(
        &mut self,
        now: crate::time::Instant,
        phy: &mut PHY,
    ) {
        let maybe_received = phy.receive_telegram();
        if maybe_received.is_some() {
            self.last_telegram_time = Some(now);
        }
        match maybe_received {
            Some(crate::fdl::Telegram::Token(token_telegram)) => {
                if token_telegram.da == self.p.address {
                    // Heyy, we got the token!
                    self.have_token = Some(now);
                } else {
                    log::trace!(
                        "Witnessed token passing: {} => {}",
                        token_telegram.sa,
                        token_telegram.da,
                    );
                }
            }
            // TODO: We must at least respond to FDL Status requests so we may at some point get
            // into the ring if other masters are present.
            Some(t) => log::trace!("Unhandled telegram: {:?}", t),
            None => (),
        }
    }

    pub fn poll<PHY: crate::phy::ProfibusPhy>(&mut self, now: crate::time::Instant, phy: &mut PHY) {
        if phy.is_transmitting() {
            self.last_telegram_time = Some(now);
            return;
        }

        if self.have_token.is_some() {
            self.handle_with_token(now, phy);
        } else {
            self.handle_without_token(now, phy);
            // We may have just received the token so do one more pass "with token".
            if self.have_token.is_some() {
                self.handle_with_token(now, phy);
            }
        }

        self.handle_lost_token(now, phy);
    }
}
