/// FDL master parameters
///
/// These parameters configure the behavior of the FDL master.
///
/// You should use the [`ParametersBuilder`] to build the parameters struct.
///
/// # Example
/// ```
/// use profirust::fdl;
/// # let buffer: [profirust::dp::PeripheralStorage; 4] = Default::default();
/// # let dp_master = profirust::dp::DpMaster::new(buffer);
///
/// let master_address = 2;
/// let param = fdl::ParametersBuilder::new(master_address, profirust::Baudrate::B19200)
///     .slot_bits(300)
///     .build_verified(&dp_master.peripherals);
/// ```
#[derive(Debug, PartialEq, Eq, Clone)]
#[non_exhaustive]
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
    /// HSA: Highest station address
    ///
    /// The HSA is only relevant for finding other masters who want to become part of the token
    /// ring.  Peripherals are allows to have addresses higher than HSA.  Masters must have an
    /// address less than HSA.
    ///
    /// **Important**: The live-list service will only detect stations below the HSA as well.
    pub highest_station_address: u8,
    /// Maximum number of retries when no answer was received
    pub max_retry_limit: u8,
    /// min T<sub>SDR</sub>: Minimum delay before anyone is allowed to respond to a telegram
    pub min_tsdr_bits: u8,
    /// Watchdog timeout for peripherals monitoring the DP master
    pub watchdog_factors: Option<(u8, u8)>,
}

impl Default for Parameters {
    fn default() -> Self {
        Parameters {
            address: 1,
            baudrate: crate::Baudrate::B19200,
            /// Tied to the baudrate - will usually be adjusted by the ParametersBuilder.
            slot_bits: 100,
            // TTR default value as found elsewhere.
            // TODO: Need to decide how to deal with this value.
            token_rotation_bits: 32436,
            /// GAP update factor, default 10 as found elsewhere.
            gap_wait_rotations: 10,
            /// 125 is the highest possible address - by default all addresses are included.
            /// (and HSA is highest address + 1)
            highest_station_address: 126,
            // Defaults to 1 byte time (= 11 bits)
            min_tsdr_bits: 11,
            // Retry limit defaults to 1, meaning that a telegram will be retried once.  This is a
            // sane default as retries should not be necessary at all on a bus that is set up
            // correctly.
            max_retry_limit: 1,
            // No watchdog by default.
            //
            // TODO: Is this what we want?  Found 6250 x HSA recommended elsewhere.
            watchdog_factors: None,
        }
    }
}

#[inline]
fn min_slot_bits(baudrate: crate::Baudrate) -> u16 {
    match baudrate {
        crate::Baudrate::B9600
        | crate::Baudrate::B19200
        | crate::Baudrate::B31250
        | crate::Baudrate::B45450
        | crate::Baudrate::B93750
        | crate::Baudrate::B187500 => 100,
        crate::Baudrate::B500000 => 200,
        crate::Baudrate::B1500000 => 300,
        crate::Baudrate::B3000000 => 400,
        crate::Baudrate::B6000000 => 600,
        crate::Baudrate::B12000000 => 1000,
    }
}

#[inline]
fn watchdog_factors(dur: crate::time::Duration) -> Option<Result<(u8, u8), ()>> {
    // TODO: Support the different watchdog time bases in some way?
    Some(dur)
        .filter(|dur| *dur != crate::time::Duration::ZERO)
        .map(|dur| {
            let timeout_10ms: u32 = (dur.total_millis() / 10).try_into().or(Err(()))?;

            for f1 in 1..256 {
                let f2 = (timeout_10ms + f1 - 1) / f1;

                if f2 < 256 {
                    return Ok((u8::try_from(f1).unwrap(), u8::try_from(f2).unwrap()));
                }
            }

            // Timeout is still too big
            Err(())
        })
}

pub struct ParametersBuilder(Parameters);

impl ParametersBuilder {
    #[inline]
    pub fn new(address: u8, baudrate: crate::Baudrate) -> Self {
        assert!(address <= 125);
        Self(Parameters {
            address,
            baudrate,
            slot_bits: min_slot_bits(baudrate),
            ..Default::default()
        })
    }

    #[inline]
    pub fn slot_bits(&mut self, slot_bits: u16) -> &mut Self {
        self.0.slot_bits = slot_bits;
        assert!(slot_bits >= min_slot_bits(self.0.baudrate));
        self
    }

    #[inline]
    pub fn highest_station_address(&mut self, hsa: u8) -> &mut Self {
        assert!(hsa > self.0.address && hsa <= 126);
        self.0.highest_station_address = hsa;
        // TODO: We probably shouldn't override an explicitly set value here...
        self.token_rotation_bits(u32::from(hsa) * 5000);
        self
    }

    pub fn token_rotation_bits(&mut self, ttr: u32) -> &mut Self {
        assert!(ttr >= 256 && ttr <= 16_777_960);
        self.0.token_rotation_bits = ttr;
        self
    }

    pub fn gap_wait_rotations(&mut self, gap_wait: u8) -> &mut Self {
        assert!(gap_wait >= 1 && gap_wait <= 100);
        self.0.gap_wait_rotations = gap_wait;
        self
    }

    #[inline]
    pub fn max_retry_limit(&mut self, max_retry_limit: u8) -> &mut Self {
        assert!(max_retry_limit >= 1 && max_retry_limit <= 15);
        self.0.max_retry_limit = max_retry_limit;
        self
    }

    #[inline]
    pub fn min_tsdr(&mut self, min_tsdr_bits: u8) -> &mut Self {
        assert!(min_tsdr_bits >= 11);
        self.0.min_tsdr_bits = min_tsdr_bits;
        self
    }

    #[inline]
    pub fn watchdog_timeout(&mut self, wdg: crate::time::Duration) -> &mut Self {
        assert!(wdg >= crate::time::Duration::from_millis(10));
        assert!(wdg <= crate::time::Duration::from_secs(650));
        self.0.watchdog_factors = watchdog_factors(wdg).transpose().unwrap();
        self
    }

    #[inline]
    pub fn build(&self) -> Parameters {
        self.0.clone()
    }

    #[inline]
    pub fn build_verified(&self, peripherals: &crate::dp::PeripheralSet) -> Parameters {
        for (_, peripheral) in peripherals.iter() {
            assert!(
                peripheral.options().max_tsdr + 15 <= self.0.slot_bits,
                "max Tsdr of peripheral #{} too large for slot time",
                peripheral.address(),
            );
        }
        self.0.clone()
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

    /// Watchdog timeout
    pub fn watchdog_timeout(&self) -> Option<crate::time::Duration> {
        self.watchdog_factors
            .map(|(f1, f2)| crate::time::Duration::from_millis(u64::from(f1) * u64::from(f2) * 10))
    }
}
