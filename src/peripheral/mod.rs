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
    fcb: crate::fdl::FrameCountBit,
    /// Process Image of Inputs
    pi_i: &'a mut [u8],
    /// Process Image of Outputs
    pi_q: &'a mut [u8],
    /// Last diagnostics request
    last_diag: Option<crate::time::Instant>,
    sent_diag: bool,
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

        let user_prm = [0x00, 0x0a, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, ];
        let module_config = [
            0xf1, // 2 word input output
        ];

        match self.state {
            PeripheralState::Reset => {
                // Request diagnostics
                Some(self.send_diagnostics_request(master, tx))
            }
            PeripheralState::WaitForParam => {
                // Send parameters
                Some(tx.send_data_telegram(
                    crate::fdl::DataTelegramHeader {
                        da: self.address,
                        sa: master.parameters().address,
                        dsap: Some(61),
                        ssap: Some(62),
                        fc: crate::fdl::FunctionCode::new_srd_low(self.fcb),
                    },
                    7 + user_prm.len(),
                    |buf| {
                        buf[0] = 0x80;
                        // WD disabled
                        buf[1] = 0x00;
                        buf[2] = 0x00;
                        // Minimum Tsdr
                        buf[3] = 11;
                        // Ident
                        buf[4] = 0x47;
                        buf[5] = 0x11;
                        // Group
                        buf[6] = 0x00;
                        // User Prm Data
                        buf[7..].copy_from_slice(&user_prm);
                    },
                ))
            }
            PeripheralState::WaitForConfig => Some(tx.send_data_telegram(
                crate::fdl::DataTelegramHeader {
                    da: self.address,
                    sa: master.parameters().address,
                    dsap: Some(62),
                    ssap: Some(62),
                    fc: crate::fdl::FunctionCode::new_srd_low(self.fcb),
                },
                module_config.len(),
                |buf| {
                    buf.copy_from_slice(&module_config);
                },
            )),
            PeripheralState::DataExchange => {
                // Request diagnostics again
                let last_diag = self.last_diag.get_or_insert(now);
                if (now - *last_diag) > crate::time::Duration::from_secs(1) {
                    *last_diag = now;
                    self.sent_diag = true;
                    Some(self.send_diagnostics_request(master, tx))
                } else {
                    Some(tx.send_data_telegram(
                        crate::fdl::DataTelegramHeader {
                            da: self.address,
                            sa: master.parameters().address,
                            dsap: crate::consts::SAP_SLAVE_DATA_EXCHANGE,
                            ssap: crate::consts::SAP_MASTER_DATA_EXCHANGE,
                            fc: crate::fdl::FunctionCode::new_srd_low(self.fcb),
                        },
                        4,
                        |buf| {
                            // buf[0] = 0x55;
                            // buf[1] = 0x55;
                            // buf[2] = 0x55;
                        },
                    ))
                }
            }
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
                if self.handle_diagnostics_response(master, &telegram) {
                    self.state = PeripheralState::WaitForParam;
                }
            }
            PeripheralState::WaitForParam => {
                if let crate::fdl::Telegram::ShortConfirmation(_) = telegram {
                    log::debug!("{} accepted parameters!", self.address);
                    self.fcb.cycle();
                    self.state = PeripheralState::WaitForConfig;
                } else {
                    todo!()
                }
            }
            PeripheralState::WaitForConfig => {
                if let crate::fdl::Telegram::ShortConfirmation(_) = telegram {
                    log::debug!("{} accepted configuration!", self.address);
                    self.fcb.cycle();
                    self.state = PeripheralState::DataExchange;
                } else {
                    todo!()
                }
            }
            PeripheralState::DataExchange => {
                if self.sent_diag {
                    self.sent_diag = false;
                    self.handle_diagnostics_response(master, &telegram);
                } else {
                    if let crate::fdl::Telegram::Data(t) = telegram {
                        log::debug!("DATA: {:?}", t.pdu);
                    }
                    self.fcb.cycle();
                }
            }
        }
    }

    pub fn send_diagnostics_request(
        &mut self,
        master: &crate::fdl::FdlMaster,
        tx: crate::fdl::TelegramTx,
    ) -> crate::fdl::TelegramTxResponse {
        tx.send_data_telegram(
            crate::fdl::DataTelegramHeader {
                da: self.address,
                sa: master.parameters().address,
                dsap: crate::consts::SAP_SLAVE_DIAGNOSIS,
                ssap: crate::consts::SAP_MASTER_MS0,
                fc: crate::fdl::FunctionCode::new_srd_low(self.fcb),
            },
            0,
            |_buf| (),
        )
    }

    pub fn handle_diagnostics_response(
        &mut self,
        master: &crate::fdl::FdlMaster,
        telegram: &crate::fdl::Telegram,
    ) -> bool {
        if let crate::fdl::Telegram::Data(t) = telegram {
            if t.h.dsap != crate::consts::SAP_MASTER_MS0 {
                log::warn!("Diagnostics response to wrong SAP: {t:?}");
                return false;
            }
            if t.h.ssap != crate::consts::SAP_SLAVE_DIAGNOSIS {
                log::warn!("Diagnostics response from wrong SAP: {t:?}");
                return false;
            }
            if t.pdu.len() < 6 {
                log::warn!("Diagnostics response too short: {t:?}");
                return false;
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
            true
        } else {
            todo!()
        }
    }
}
