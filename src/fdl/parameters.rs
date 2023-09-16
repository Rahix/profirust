/// FDL master parameters
///
/// These parameters configure the behavior of the FDL master.
///
/// # Example
/// ```
/// use profirust::fdl;
///
/// let param = fdl::Parameters {
///     address: 2,
///     baudrate: profirust::Baudrate::B31250,
///     .. Default::default()
/// };
/// ```
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Parameters {
    /// Station address for this master
    pub address: u8,
    /// Baudrate
    pub baudrate: crate::Baudrate,
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
    /// min T<sub>SDR</sub>: Minimum delay before anyone is allowed to respond to a telegram
    pub min_tsdr_bits: u8,
}

impl Default for Parameters {
    fn default() -> Self {
        Parameters {
            address: 1,
            baudrate: crate::Baudrate::B19200,
            slot_bits: 100, // TODO: needs to be adjusted depending on baudrate
            token_rotation_bits: 20000, // TODO: really sane default?  This was at least recommended somewhere...
            gap_wait_rotations: 100,    // TODO: sane default?
            highest_station_address: 125,
            // Defaults to 1 byte time (= 11 bits)
            min_tsdr_bits: 11,
            // Retry limit defaults to 1, meaning that a telegram will be retried once.  This is a
            // sane default as retries should not be necessary at all on a bus that is set up
            // correctly.
            max_retry_limit: 1,
        }
    }
}

impl Parameters {
    pub fn bits_to_time(&self, bits: u32) -> crate::time::Duration {
        self.baudrate.bits_to_time(bits)
    }

    /// T<sub>SL</sub> (slot time) converted to duration
    pub fn slot_time(&self) -> crate::time::Duration {
        self.bits_to_time(u32::from(self.slot_bits))
    }

    /// min T<sub>SDR</sub> (minimum time before responding) converted to duration
    pub fn min_tsdr_time(&self) -> crate::time::Duration {
        self.bits_to_time(u32::from(self.min_tsdr_bits))
    }

    /// Timeout after which the token is considered lost.
    ///
    /// Calculated as 6 * T<sub>SL</sub> + 2 * Addr * T<sub>SL</sub>.
    pub fn token_lost_timeout(&self) -> crate::time::Duration {
        let timeout_bits = u32::from(self.slot_bits) * (6 + 2 * u32::from(self.address));
        self.bits_to_time(timeout_bits)
    }

    /// T<sub>TR</sub> (projected token rotation time)
    pub fn token_rotation_time(&self) -> crate::time::Duration {
        self.bits_to_time(self.token_rotation_bits)
    }
}
