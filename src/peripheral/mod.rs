#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
enum FrameCountBit {
    #[default]
    NotValid,
    High,
    Low,
}

impl FrameCountBit {
    pub fn reset(&mut self) {
        *self = FrameCountBit::NotValid;
    }

    pub fn cycle(&mut self) {
        *self = match self {
            FrameCountBit::NotValid => FrameCountBit::Low,
            FrameCountBit::High => FrameCountBit::Low,
            FrameCountBit::Low => FrameCountBit::High,
        }
    }

    pub fn fcb(self) -> bool {
        match self {
            FrameCountBit::NotValid => true,
            FrameCountBit::High => true,
            FrameCountBit::Low => false,
        }
    }

    pub fn fcv(self) -> bool {
        match self {
            FrameCountBit::NotValid => false,
            FrameCountBit::High => true,
            FrameCountBit::Low => true,
        }
    }

    pub fn make_request_fc(self, req: crate::fdl::RequestType) -> crate::fdl::FunctionCode {
        crate::fdl::FunctionCode::Request {
            fcb: self.fcb(),
            fcv: self.fcv(),
            req,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
enum PeripheralState {
    #[default]
    Offline,
    Reset,
    WaitForParam,
    WaitForConfig,
    DataExchange,
}

#[derive(Debug, PartialEq, Eq, Default)]
pub struct Peripheral<'a> {
    /// Station address of this peripheral (slave)
    address: u8,
    /// Current state of this peripheral
    state: PeripheralState,
    /// FCB/FCV tracking for this peripheral
    ///
    /// The "Frame Count Bit" is used to detect lost messages and prevent duplication on either
    /// side.
    fcb: FrameCountBit,
    /// Process Image of Inputs
    pi_i: &'a mut [u8],
    /// Process Image of Outputs
    pi_q: &'a mut [u8],
}

impl<'a> Peripheral<'a> {
    pub fn new(address: u8) -> Self {
        Self {
            address,
            ..Default::default()
        }
    }

    /// Address of this peripheral.
    #[inline(always)]
    pub fn address(&self) -> u8 {
        self.address
    }

    /// Access to the full process image of inputs.
    #[inline(always)]
    pub fn pi_i(&self) -> &[u8] {
        &self.pi_i
    }

    /// Access to the full process image of outputs.
    #[inline(always)]
    pub fn pi_q(&self) -> &[u8] {
        &self.pi_q
    }

    /// Mutable access to the full process image of outputs.
    #[inline(always)]
    pub fn pi_q_mut(&mut self) -> &mut [u8] {
        &mut self.pi_q
    }

    /// Whether this peripheral is live and responds on the bus.
    #[inline(always)]
    pub fn is_live(&self) -> bool {
        self.state != PeripheralState::Offline
    }

    /// Whether this peripheral is live and exchanging data with us.
    #[inline(always)]
    pub fn is_running(&self) -> bool {
        self.state == PeripheralState::DataExchange
    }
}

impl<'a> Peripheral<'a> {
    pub fn try_start_message_cycle(
        &mut self,
        now: crate::time::Instant,
        master: &crate::fdl::FdlMaster,
        tx: crate::fdl::TelegramTx,
        high_prio_only: bool,
    ) -> Option<crate::fdl::TelegramTxResponse> {
        if !master.check_address_live(self.address) {
            self.state = PeripheralState::Offline;
            return None;
        } else if self.state == PeripheralState::Offline {
            // Live but we're still "offline" => go to "reset" state
            self.state = PeripheralState::Reset;
        }

        match self.state {
            PeripheralState::Reset => {
                // Request diagnostics
                Some(tx.send_data_telegram(
                    crate::fdl::DataTelegramHeader {
                        da: self.address,
                        sa: master.parameters().address,
                        dsap: Some(60),
                        ssap: Some(62),
                        fc: self.fcb.make_request_fc(crate::fdl::RequestType::SrdLow),
                    },
                    0,
                    |_buf| (),
                ))
            }
            PeripheralState::WaitForParam => todo!(),
            PeripheralState::WaitForConfig => todo!(),
            PeripheralState::DataExchange => todo!(),
            PeripheralState::Offline => unreachable!(),
        }
    }

    pub fn handle_response(
        &mut self,
        now: crate::time::Instant,
        master: &crate::fdl::FdlMaster,
        telegram: crate::fdl::Telegram,
    ) {
        match self.state {
            PeripheralState::Offline => unreachable!(),
            PeripheralState::Reset => {
                // Diagnostics response
                if let crate::fdl::Telegram::Data(t) = telegram {
                    if t.h.dsap != Some(62) {
                        log::warn!("Diagnostics response to wrong SAP: {t:?}");
                        return;
                    }
                    if t.h.ssap != Some(60) {
                        log::warn!("Diagnostics response from wrong SAP: {t:?}");
                        return;
                    }
                    if t.pdu.len() < 6 {
                        log::warn!("Diagnostics response too short: {t:?}");
                        return;
                    }

                    let p_addr = self.address;
                    let real_master = master.parameters().address;

                    let station_not_ready = (t.pdu[0] & (1 << 1)) != 0;
                    let config_fault = (t.pdu[0] & (1 << 2)) != 0;
                    let ext_diag = (t.pdu[0] & (1 << 3)) != 0;
                    let function_not_supported = (t.pdu[0] & (1 << 4)) != 0;
                    let param_fault = (t.pdu[0] & (1 << 6)) != 0;

                    let param_req = (t.pdu[1] & (1 << 0)) != 0;
                    let stat_diag = (t.pdu[1] & (1 << 1)) != 0;
                    let perm_on = (t.pdu[1] & (1 << 2)) != 0;
                    if !perm_on {
                        log::warn!("Inconsistent diagnostics!");
                    }
                    let watchdog_on = (t.pdu[1] & (1 << 3)) != 0;
                    let freeze_mode = (t.pdu[1] & (1 << 4)) != 0;
                    let sync_mode = (t.pdu[1] & (1 << 5)) != 0;

                    let master_address = t.pdu[3];
                    let ident_high = t.pdu[4];
                    let ident_low = t.pdu[5];

                    log::info!(
                        r#"Peripheral Diagnostics (#{p_addr}):
 - Station Not Ready:       {station_not_ready}
 - Config Fault:            {config_fault}
 - Extended Diagnostics:    {ext_diag}
 - Function Not Supp.:      {function_not_supported}
 - Parameter Fault:         {param_fault}
 - Parameters Required:     {param_req}
 - Station Diagnostics:     {stat_diag}
 - Watchdog On:             {watchdog_on}
 - FREEZE Mode:             {freeze_mode}
 - SYNC Mode:               {sync_mode}
 - Master Address:          {master_address} (we are {real_master})
 - Ident:                   {ident_high:02x} {ident_low:02x}
 "#
                    );

                    self.fcb.cycle();
                    self.state = PeripheralState::WaitForParam;
                }
            }
            PeripheralState::WaitForParam => todo!(),
            PeripheralState::WaitForConfig => todo!(),
            PeripheralState::DataExchange => todo!(),
        }
    }
}
