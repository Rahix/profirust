/// Operating state of the FDL master
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum OperatingState {
    /// The DP master is part of the token ring but not performing any cyclic data exchange.
    Stop,
    /// All peripherals/slaves are initialized and blocked.  Cyclic data exchange is performed, but
    /// not outputs are written.
    Clear,
    /// Regular operation.  All peripherals/slaves are initialized and blocked.  Cyclic data
    /// exchange is performed with full I/O.
    Operate,
}

impl OperatingState {
    #[inline(always)]
    pub fn is_stop(self) -> bool {
        self == OperatingState::Stop
    }

    #[inline(always)]
    pub fn is_clear(self) -> bool {
        self == OperatingState::Clear
    }

    #[inline(always)]
    pub fn is_operate(self) -> bool {
        self == OperatingState::Operate
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
enum CycleState {
    /// Currently exchanging data with peripheral at the given index.
    ///
    /// **Important**: This is **not** the address, but the internal peripheral index.
    DataExchange(u8),
    /// State to indicate the a full data exchange cycle has been completed.
    CycleCompleted,
}

/// The DP master.
///
/// Currently only implements a subset of DP-V0.
///
/// The DP master holds all peripherals that we interact with.  To get access, use the
/// [`PeripheralHandle`] that you get when calling [`.add()`][`DpMaster::add`].
#[non_exhaustive]
pub struct DpMaster<'a> {
    pub peripherals: crate::dp::PeripheralSet<'a>,
    pub state: DpMasterState,
}

pub struct DpMasterState {
    /// Operating State of the master.
    pub operating_state: OperatingState,

    /// Last time we sent a "Global Control" telegram to advertise our operating state.
    pub last_global_control: Option<crate::time::Instant>,

    /// Cycle State, tracking progress of the data exchange cycle
    cycle_state: CycleState,
}

impl<'a> DpMaster<'a> {
    pub fn new<S>(storage: S) -> Self
    where
        S: Into<managed::ManagedSlice<'a, crate::dp::PeripheralStorage<'a>>>,
    {
        let storage = storage.into();
        if storage.len() > 124 {
            log::warn!("DP master was provided with storage for more than 124 peripherals, this is wasted memory!");
        }
        Self {
            peripherals: crate::dp::PeripheralSet::new(storage),
            state: DpMasterState {
                operating_state: OperatingState::Stop,
                last_global_control: None,
                cycle_state: CycleState::DataExchange(0),
            },
        }
    }

    fn increment_cycle_state(&mut self, index: u8) {
        if let Some(next) = self.peripherals.get_next_index(index) {
            self.state.cycle_state = CycleState::DataExchange(next);
        } else {
            self.state.cycle_state = CycleState::CycleCompleted;
        }
    }
}

impl DpMasterState {
    #[inline(always)]
    pub fn operating_state(&self) -> OperatingState {
        self.operating_state
    }

    #[inline]
    pub fn enter_state(&mut self, state: OperatingState) {
        log::info!("DP master entering state \"{:?}\"", state);
        self.operating_state = state;
        // Ensure we will send a new global control telegram ASAP:
        self.last_global_control = None;

        if state != OperatingState::Operate {
            todo!("OperatingState {:?} is not yet supported properly!", state);
        }
    }

    /// Enter the [`Stop`][`OperatingState::Stop`] operating state.
    ///
    /// This is equivalent to calling `.enter_state(OperatingState::Stop)`.
    #[inline]
    pub fn enter_stop(&mut self) {
        self.enter_state(OperatingState::Stop)
    }

    /// Enter the [`Clear`][`OperatingState::Clear`] operating state.
    ///
    /// This is equivalent to calling `.enter_state(OperatingState::Clear)`.
    #[inline]
    pub fn enter_clear(&mut self) {
        self.enter_state(OperatingState::Clear)
    }

    /// Enter the [`Operate`][`OperatingState::Operate`] operating state.
    ///
    /// This is equivalent to calling `.enter_state(OperatingState::Operate)`.
    #[inline]
    pub fn enter_operate(&mut self) {
        self.enter_state(OperatingState::Operate)
    }

    /// Whether the DP bus cycle was completed during the last poll.
    #[inline]
    pub fn cycle_completed(&self) -> bool {
        self.cycle_state == CycleState::CycleCompleted
    }
}

impl<'a> crate::fdl::FdlApplication for DpMaster<'a> {
    fn transmit_telegram(
        &mut self,
        now: crate::time::Instant,
        fdl: &crate::fdl::FdlMaster,
        mut tx: crate::fdl::TelegramTx,
        high_prio_only: bool,
    ) -> Option<crate::fdl::TelegramTxResponse> {
        // In STOP state, never send anything
        if self.state.operating_state.is_stop() {
            return None;
        }

        // First check whether it is time for another global control telegram
        //
        // TODO: 50 Tsl is an arbitrary interval.  Documentation talks about 3 times the watchdog
        // period, but that seems rather arbitrary as well.
        if !high_prio_only
            && self
                .state
                .last_global_control
                .map(|t| now - t >= fdl.parameters().slot_time() * 50)
                .unwrap_or(true)
        {
            self.state.last_global_control = Some(now);
            log::trace!(
                "DP master sending global control for state {:?}",
                self.state.operating_state
            );
            return Some(tx.send_data_telegram(
                crate::fdl::DataTelegramHeader {
                    da: 0x7f,
                    sa: fdl.parameters().address,
                    dsap: crate::consts::SAP_SLAVE_GLOBAL_CONTROL,
                    ssap: crate::consts::SAP_MASTER_MS0,
                    fc: crate::fdl::FunctionCode::Request {
                        // TODO: Do we need an FCB for GC telegrams?
                        fcb: crate::fdl::FrameCountBit::Inactive,
                        req: crate::fdl::RequestType::SdnLow,
                    },
                },
                2,
                |buf| {
                    buf[0] = match self.state.operating_state {
                        OperatingState::Clear => 0x02,
                        OperatingState::Operate => 0x00,
                        OperatingState::Stop => unreachable!(),
                    };
                    buf[1] = 0x00;
                },
            ));
        }

        loop {
            let index = match self.state.cycle_state {
                CycleState::DataExchange(i) => i,
                CycleState::CycleCompleted => {
                    // On CycleCompleted, return None to let the FDL know where done.  Reset the
                    // cycle state to the beginning for the next time.
                    self.state.cycle_state = CycleState::DataExchange(0);
                    return None;
                }
            };

            if let Some((handle, peripheral)) = self.peripherals.get_at_index_mut(index) {
                let res = peripheral.transmit_telegram(now, &self.state, fdl, tx, high_prio_only);

                match res {
                    Ok(tx_res) => {
                        // When this peripheral initiated a transmission, break out of the loop
                        return Some(tx_res);
                    }
                    Err(tx_returned) => {
                        // When this peripheral was not interested in sending data, move on to the
                        // next one.
                        self.increment_cycle_state(index);
                        tx = tx_returned;
                    }
                }
            }
        }
    }

    fn receive_reply(
        &mut self,
        now: crate::time::Instant,
        fdl: &crate::fdl::FdlMaster,
        addr: u8,
        telegram: crate::fdl::Telegram,
    ) {
        let index = match self.state.cycle_state {
            CycleState::DataExchange(i) => i,
            CycleState::CycleCompleted => {
                unreachable!("impossible to get a reply when the cycle was completed!");
            }
        };
        match self.peripherals.get_at_index_mut(index) {
            Some((handle, peripheral)) if addr == peripheral.address() => {
                peripheral.receive_reply(now, &self.state, fdl, telegram);
                self.increment_cycle_state(index);
            }
            _ => {
                unreachable!(
                    "Received reply for unknown/unexpected peripheral #{addr}: {telegram:?}"
                );
            }
        }
    }

    fn handle_timeout(&mut self, now: crate::time::Instant, fdl: &crate::fdl::FdlMaster, addr: u8) {
        // At this time, there is no meaningful action to take in response to this.  Timeout
        // handling is actually done as part of the transmit_telegram() code.
        //
        // log::warn!("Timeout while waiting for response from #{}!", addr);
    }
}
