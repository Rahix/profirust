#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Parameters {
    /// Station address for this master
    pub address: u8,
    /// Baudrate
    pub baudrate: crate::fdl::Baudrate,
    /// Slot time
    pub t_sl: u16,
    /// Planned token circulation time
    pub ttr: u32,
    /// GAP update factor (how many token rotations to wait before polling the gap again)
    pub gap: u8,
    /// Highest projected station address
    pub hsa: u8,
    /// Maximum number of retries when no answer was received
    pub max_retry_limit: u8,
}

impl Default for Parameters {
    fn default() -> Self {
        Parameters {
            address: 1,
            baudrate: crate::fdl::Baudrate::B19200,
            t_sl: 100,
            ttr: 20000,
            gap: 10, // TODO: sane default?
            hsa: 125,
            max_retry_limit: 6, // TODO: sane default?
        }
    }
}

#[derive(Debug)]
pub struct FdlMaster<'a> {
    comm_buffer: Option<crate::phy::BufferHandle<'a>>,
    param: Parameters,

    last_telegram_time: Option<crate::time::Instant>,
}

impl<'a> FdlMaster<'a> {
    pub fn new<B: Into<crate::phy::BufferHandle<'a>>>(buffer: B, param: Parameters) -> Self {
        Self {
            comm_buffer: Some(buffer.into()),
            param,
            last_telegram_time: None,
        }
    }

    pub fn poll<'b, PHY: crate::phy::ProfibusPhy<'b>>(
        &mut self,
        timestamp: crate::time::Instant,
        phy: &mut PHY,
    ) {
        let last_telegram_time = *self.last_telegram_time.get_or_insert(timestamp);
        let token_lost_timeout = self.param.baudrate.bits_to_time(
            6 * self.param.t_sl as u32 + 2 * self.param.address as u32 * self.param.t_sl as u32,
        );
        log::debug!("token_lost_timeout = {:?}", token_lost_timeout);
        if (timestamp - last_telegram_time) >= (token_lost_timeout) {
            log::warn!("Token lost!  Generating a new one.");
            let token_telegram =
                crate::fdl::TokenTelegram::new(self.param.address, self.param.address);
        }
    }
}
