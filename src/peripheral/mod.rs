#[derive(Debug, PartialEq, Eq, Default)]
pub struct PeripheralOptions<'a> {
    pub ident_number: u16,

    pub sync_mode: bool,
    pub freeze_mode: bool,
    pub groups: u8,
    pub watchdog: Option<(u8, u8)>,

    pub user_parameters: Option<&'a [u8]>,
    pub config: Option<&'a [u8]>,
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
    fcb: crate::fdl::FrameCountBit,
    /// Process Image of Inputs
    pi_i: &'a mut [u8],
    /// Process Image of Outputs
    pi_q: &'a mut [u8],
    /// Last diagnostics request
    last_diag: Option<crate::time::Instant>,
    sent_diag: bool,

    options: PeripheralOptions<'a>,
}

impl<'a> Peripheral<'a> {
    pub fn new(
        address: u8,
        options: PeripheralOptions<'a>,
        pi_i: &'a mut [u8],
        pi_q: &'a mut [u8],
    ) -> Self {
        Self {
            address,
            options,
            pi_i,
            pi_q,
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
                Some(self.send_diagnostics_request(master, tx))
            }
            PeripheralState::WaitForParam => {
                if let Some(user_parameters) = self.options.user_parameters {
                    // Send parameters
                    Some(tx.send_data_telegram(
                        crate::fdl::DataTelegramHeader {
                            da: self.address,
                            sa: master.parameters().address,
                            dsap: crate::consts::SAP_SLAVE_SET_PRM,
                            ssap: crate::consts::SAP_MASTER_MS0,
                            fc: crate::fdl::FunctionCode::new_srd_low(self.fcb),
                        },
                        7 + user_parameters.len(),
                        |buf| {
                            // Construct Station Status Byte
                            buf[0] |= 0x80; // Lock_Req
                            if self.options.sync_mode {
                                buf[0] |= 0x20; // Sync_Req
                            }
                            if self.options.freeze_mode {
                                buf[0] |= 0x10; // Freeze_Req
                            }
                            if let Some((f1, f2)) = self.options.watchdog {
                                buf[0] |= 0x08; // WD_On
                                buf[1] = f1;
                                buf[2] = f2;
                            }
                            // Minimum T_sdr
                            buf[3] = 11;
                            // Ident
                            buf[4..6].copy_from_slice(&self.options.ident_number.to_be_bytes());
                            // Groups
                            buf[6] = self.options.groups;
                            // User Prm Data
                            buf[7..].copy_from_slice(&user_parameters);
                        },
                    ))
                } else {
                    // When self.options.user_parameters is None, we need to wait before we can
                    // start with configuration.
                    None
                }
            }
            PeripheralState::WaitForConfig => {
                if let Some(config) = self.options.config {
                    Some(tx.send_data_telegram(
                        crate::fdl::DataTelegramHeader {
                            da: self.address,
                            sa: master.parameters().address,
                            dsap: crate::consts::SAP_SLAVE_CHK_CFG,
                            ssap: crate::consts::SAP_MASTER_MS0,
                            fc: crate::fdl::FunctionCode::new_srd_low(self.fcb),
                        },
                        config.len(),
                        |buf| {
                            buf.copy_from_slice(&config);
                        },
                    ))
                } else {
                    // When self.options.config is None, we need to wait before we can start with
                    // configuration.
                    None
                }
            }
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
                        self.pi_q.len(),
                        |buf| buf.copy_from_slice(&self.pi_q),
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
                        if t.pdu.len() == self.pi_i.len() {
                            self.pi_i.copy_from_slice(&t.pdu);
                        } else {
                            log::warn!("Got response with unexpected pdu length!");
                        }
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
