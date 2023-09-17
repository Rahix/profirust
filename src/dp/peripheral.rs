#[derive(Debug, PartialEq, Eq, Default)]
pub struct PeripheralOptions<'a> {
    pub ident_number: u16,

    pub sync_mode: bool,
    pub freeze_mode: bool,
    pub groups: u8,
    pub max_tsdr: u16,
    pub fail_safe: bool,

    pub user_parameters: Option<&'a [u8]>,
    pub config: Option<&'a [u8]>,
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct DiagnosticFlags: u16 {
        // const STATION_NON_EXISTENT = 0b00000001;
        const STATION_NOT_READY =       0b00000010;
        const CONFIGURATION_FAULT =     0b00000100;
        const EXT_DIAG =                0b00001000;
        const NOT_SUPPORTED =           0b00010000;
        // const INVALID_RESPONSE =     0b00100000;
        const PARAMETER_FAULT =         0b01000000;
        // const MASTER_LOCK =          0b10000000;

        const PARAMETER_REQUIRED =      0b00000001_00000000;
        const STATUS_DIAGNOSTICS =      0b00000010_00000000;
        const PERMANENT_BIT =           0b00000100_00000000;
        const WATCHDOG_ON =             0b00001000_00000000;
        const FREEZE_MODE =             0b00010000_00000000;
        const SYNC_MODE =               0b00100000_00000000;
        // const RESERVED =             0b01000000_00000000;
        // const DEACTIVATED =          0b10000000_00000000;
    }
}

/// Events that can occur while communicating with a peripheral.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum PeripheralEvent {
    /// Peripheral went online and started responding to messages.
    Online,
    /// Peripheral accepted parameters and configuration and is now ready for data exchange.
    Configured,
    /// Peripheral rejected configuration and needs to be re-configured.
    ConfigError,
    /// Peripheral rejected parameters and needs to be re-parameterized.
    ParameterError,
    /// Cyclic data exchange with this peripheral completed and the PI<sub>I</sub> and
    /// PI<sub>Q</sub> have been updated.
    DataExchanged,
    /// Peripheral has new diagnostic data available.
    Diagnostics,
    /// Peripheral stopped responding to messages.
    Offline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeripheralDiagnostics {
    pub flags: DiagnosticFlags,
    pub ident_number: u16,
    pub master_address: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
enum PeripheralState {
    #[default]
    Offline,
    WaitForParam,
    WaitForConfig,
    ValidateConfig,
    PreDataExchange,
    DataExchange,
}

#[derive(Debug, PartialEq, Eq, Default)]
pub struct Peripheral<'a> {
    /// Station address of this peripheral (slave)
    address: u8,
    /// Current state of this peripheral
    state: PeripheralState,
    /// Retry count when messages don't receive a valid response.
    retry_count: u8,
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
    diag: Option<PeripheralDiagnostics>,

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

    #[inline(always)]
    pub fn options(&self) -> &PeripheralOptions<'a> {
        &self.options
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

    /// Access to the process images of inputs (immutable) and outputs (mutable).
    pub fn pi_both(&mut self) -> (&[u8], &mut [u8]) {
        (&self.pi_i, &mut self.pi_q)
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

    /// Get the last diagnostics information received from this peripheral.
    #[inline]
    pub fn last_diagnostics(&self) -> Option<&PeripheralDiagnostics> {
        self.diag.as_ref()
    }
}

impl<'a> Peripheral<'a> {
    pub fn transmit_telegram<'b>(
        &mut self,
        now: crate::time::Instant,
        dp: &crate::dp::DpMasterState,
        fdl: &crate::fdl::FdlMaster,
        tx: crate::fdl::TelegramTx<'b>,
        high_prio_only: bool,
    ) -> Result<crate::fdl::TelegramTxResponse, (crate::fdl::TelegramTx<'b>, Option<PeripheralEvent>)>
    {
        // We never expect to be called in `Stop` or even worse `Offline` operating states.
        debug_assert!(dp.operating_state.is_operate() || dp.operating_state.is_clear());

        if self.state != PeripheralState::Offline && self.retry_count == 1 {
            log::warn!("Resending a telegram to #{}...", self.address);
        }

        let res = match self.state {
            _ if self.retry_count > fdl.parameters().max_retry_limit => {
                // Assume peripheral is now offline so the next step is sending SYNC messages to detect
                // when it comes back.
                log::warn!("Peripheral #{} stopped responding!", self.address);
                self.state = PeripheralState::Offline;
                Err((tx, Some(PeripheralEvent::Offline)))
            }
            PeripheralState::Offline => {
                if self.retry_count == 0 {
                    // Request diagnostics to see whether the peripheral responds.
                    Ok(self.send_diagnostics_request(fdl, tx))
                } else {
                    // Don't retry when the peripheral may be offline.
                    Err((tx, None))
                }
            }
            PeripheralState::WaitForParam => {
                if let Some(user_parameters) = self.options.user_parameters {
                    // Send parameters
                    Ok(tx.send_data_telegram(
                        crate::fdl::DataTelegramHeader {
                            da: self.address,
                            sa: fdl.parameters().address,
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
                            if let Some((f1, f2)) = fdl.parameters().watchdog_factors {
                                buf[0] |= 0x08; // WD_On
                                buf[1] = f1;
                                buf[2] = f2;
                            }
                            // Minimum T_sdr
                            buf[3] = fdl.parameters().min_tsdr_bits;
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
                    Err((tx, None))
                }
            }
            PeripheralState::WaitForConfig => {
                if let Some(config) = self.options.config {
                    Ok(tx.send_data_telegram(
                        crate::fdl::DataTelegramHeader {
                            da: self.address,
                            sa: fdl.parameters().address,
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
                    Err((tx, None))
                }
            }
            PeripheralState::ValidateConfig => {
                // Request diagnostics once more
                Ok(self.send_diagnostics_request(fdl, tx))
            }
            PeripheralState::DataExchange | PeripheralState::PreDataExchange => {
                Ok(tx.send_data_telegram(
                    crate::fdl::DataTelegramHeader {
                        da: self.address,
                        sa: fdl.parameters().address,
                        dsap: crate::consts::SAP_SLAVE_DATA_EXCHANGE,
                        ssap: crate::consts::SAP_MASTER_DATA_EXCHANGE,
                        fc: crate::fdl::FunctionCode::new_srd_high(self.fcb),
                    },
                    self.pi_q.len(),
                    |buf| {
                        // Only write output process image in `Operate` state.  In `Clear`
                        // state, we leave the output process image all zeros.
                        if dp.operating_state.is_operate() {
                            buf.copy_from_slice(&self.pi_q);
                        }
                    },
                ))
            }
        };

        // When we are transmitting a telegram, increment the retry count.
        if res.is_ok() {
            self.retry_count += 1;
        } else {
            self.retry_count = 0;
        }

        res
    }

    pub fn receive_reply(
        &mut self,
        now: crate::time::Instant,
        dp: &crate::dp::DpMasterState,
        fdl: &crate::fdl::FdlMaster,
        telegram: crate::fdl::Telegram,
    ) -> Option<PeripheralEvent> {
        match self.state {
            PeripheralState::Offline => {
                // Diagnostics response
                if self.handle_diagnostics_response(fdl, &telegram).is_some() {
                    self.retry_count = 0;
                    self.state = PeripheralState::WaitForParam;
                    Some(PeripheralEvent::Online)
                } else {
                    None
                }
            }
            PeripheralState::WaitForParam => {
                if let crate::fdl::Telegram::ShortConfirmation(_) = telegram {
                    log::debug!("Sent parameters to #{}.", self.address);
                    self.fcb.cycle();
                    self.state = PeripheralState::WaitForConfig;
                    self.retry_count = 0;
                    None
                } else {
                    todo!()
                }
            }
            PeripheralState::WaitForConfig => {
                if let crate::fdl::Telegram::ShortConfirmation(_) = telegram {
                    log::debug!("Sent configuration to #{}.", self.address);
                    self.fcb.cycle();
                    self.state = PeripheralState::ValidateConfig;
                    self.retry_count = 0;
                    None
                } else {
                    todo!()
                }
            }
            PeripheralState::ValidateConfig => {
                let address = self.address;
                self.retry_count = 0;
                let (new_state, event) =
                    if let Some(diag) = self.handle_diagnostics_response(fdl, &telegram) {
                        if diag.flags.contains(DiagnosticFlags::PARAMETER_FAULT) {
                            log::warn!("Peripheral #{} reports a parameter fault!", address);
                            // TODO: Going to `Offline` here will just end in a loop.
                            (
                                PeripheralState::Offline,
                                Some(PeripheralEvent::ParameterError),
                            )
                        } else if diag.flags.contains(DiagnosticFlags::CONFIGURATION_FAULT) {
                            log::warn!("Peripheral #{} reports a configuration fault!", address);
                            // TODO: Going to `Offline` here will just end in a loop.
                            (PeripheralState::Offline, Some(PeripheralEvent::ConfigError))
                        } else if diag.flags.contains(DiagnosticFlags::PARAMETER_REQUIRED) {
                            log::warn!(
                            "Peripheral #{} wants parameters after completing setup?! Retrying...",
                            address
                        );
                            // TODO: Report an event here?
                            (PeripheralState::WaitForParam, None)
                        } else if !diag.flags.contains(DiagnosticFlags::STATION_NOT_READY) {
                            log::info!("Peripheral #{} becomes ready for data exchange.", address);
                            (
                                PeripheralState::PreDataExchange,
                                Some(PeripheralEvent::Configured),
                            )
                        } else {
                            (PeripheralState::ValidateConfig, None)
                        }
                    } else {
                        (PeripheralState::ValidateConfig, None)
                    };
                self.state = new_state;
                event
            }
            PeripheralState::DataExchange | PeripheralState::PreDataExchange => {
                let event = if let crate::fdl::Telegram::Data(t) = telegram {
                    let data_ok = match t.is_response().unwrap() {
                        crate::fdl::ResponseStatus::SapNotEnabled => {
                            log::warn!(
                                "Got \"SAP not enabled\" response from #{}, revalidating config...",
                                self.address
                            );
                            self.state = PeripheralState::ValidateConfig;
                            false
                        }

                        crate::fdl::ResponseStatus::Ok => true, // TODO: Is this actually correct?
                        crate::fdl::ResponseStatus::DataLow => true,
                        crate::fdl::ResponseStatus::DataHigh => true,

                        e => {
                            log::warn!(
                                "Unhandled response status \"{:?}\" from #{}!",
                                e,
                                self.address
                            );
                            false
                        }
                    };

                    if data_ok {
                        if t.pdu.len() == self.pi_i.len() {
                            self.pi_i.copy_from_slice(&t.pdu);
                            self.state = PeripheralState::DataExchange;
                            Some(PeripheralEvent::DataExchanged)
                        } else {
                            log::warn!(
                            "Got response from #{} with unexpected PDU length (got: {}, want: {})!",
                            self.address,
                            t.pdu.len(),
                            self.pi_i.len()
                        );
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
                self.retry_count = 0;
                self.fcb.cycle();
                event
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
    ) -> Option<&PeripheralDiagnostics> {
        if let crate::fdl::Telegram::Data(t) = telegram {
            if t.h.dsap != crate::consts::SAP_MASTER_MS0 {
                log::warn!(
                    "Diagnostics response by #{} to wrong SAP: {t:?}",
                    self.address
                );
                return None;
            }
            if t.h.ssap != crate::consts::SAP_SLAVE_DIAGNOSIS {
                log::warn!(
                    "Diagnostics response by #{} from wrong SAP: {t:?}",
                    self.address
                );
                return None;
            }
            if t.pdu.len() < 6 {
                log::warn!(
                    "Diagnostics response by #{} is too short: {t:?}",
                    self.address
                );
                return None;
            }

            let mut diag = PeripheralDiagnostics {
                flags: DiagnosticFlags::from_bits_retain(u16::from_le_bytes(
                    t.pdu[0..2].try_into().unwrap(),
                )),
                master_address: t.pdu[3],
                ident_number: u16::from_be_bytes(t.pdu[4..6].try_into().unwrap()),
            };

            if !diag.flags.contains(DiagnosticFlags::PERMANENT_BIT) {
                log::warn!("Inconsistent diagnostics for peripheral #{}!", self.address);
            }
            // we don't need the permanent bit anymore now
            diag.flags.remove(DiagnosticFlags::PERMANENT_BIT);

            log::debug!("Peripheral Diagnostics (#{}): {:?}", self.address, diag);

            self.fcb.cycle();

            self.diag = Some(diag);
            self.diag.as_ref()
        } else {
            todo!()
        }
    }
}
