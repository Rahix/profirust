use crate::dp::Peripheral;
use core::fmt;

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

/// The DP master.
///
/// Currently only implements a subset of DP-V0.
///
/// The DP master holds all peripherals that we interact with.  To get access, use the
/// [`PeripheralHandle`] that you get when calling [`.add()`][`DpMaster::add`].
pub struct DpMaster<'a> {
    peripherals: crate::dp::PeripheralSet<'a>,
    state: DpMasterState,
}

pub struct DpMasterState {
    /// Operating State of the master.
    pub operating_state: OperatingState,

    /// Last time we sent a "Global Control" telegram to advertise our operating state.
    pub last_global_control: Option<crate::time::Instant>,
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
            },
        }
    }

    /// Add a peripheral to the set, and return its handle.
    ///
    /// # Panics
    /// This function panics if the storage is fixed-size (not a `Vec`) and is full.
    pub fn add(&mut self, peripheral: Peripheral<'a>) -> crate::dp::PeripheralHandle {
        self.peripherals.add(peripheral)
    }

    /// Get a peripheral from the set by its handle, as mutable.
    ///
    /// # Panics
    /// This function may panic if the handle does not belong to this peripheral set.
    pub fn get_mut(&mut self, handle: crate::dp::PeripheralHandle) -> &mut Peripheral<'a> {
        self.peripherals.get_mut(handle)
    }

    pub fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = (crate::dp::PeripheralHandle, &mut Peripheral<'a>)> {
        self.peripherals.iter_mut()
    }

    #[inline(always)]
    pub fn operating_state(&self) -> OperatingState {
        self.state.operating_state
    }

    #[inline]
    pub fn enter_state(&mut self, state: OperatingState) {
        log::info!("DP master entering state \"{:?}\"", state);
        self.state.operating_state = state;
        // Ensure we will send a new global control telegram ASAP:
        self.state.last_global_control = None;

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
}

impl<'a> crate::fdl::FdlApplication for DpMaster<'a> {
    fn transmit_telegram(
        &mut self,
        now: crate::time::Instant,
        fdl: &crate::fdl::FdlMaster,
        tx: crate::fdl::TelegramTx,
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

        // TODO: naive implementation that only works with one peripheral.
        self.peripherals
            .iter_mut()
            .next()
            .and_then(|(_, peripheral)| {
                peripheral
                    .try_start_message_cycle(now, &self.state, fdl, tx, high_prio_only)
                    .ok()
            })
    }

    fn receive_reply(
        &mut self,
        now: crate::time::Instant,
        fdl: &crate::fdl::FdlMaster,
        addr: u8,
        telegram: crate::fdl::Telegram,
    ) {
        for (_, peripheral) in self.peripherals.iter_mut() {
            if peripheral.address() == addr {
                peripheral.handle_response(now, &self.state, fdl, telegram);
                return;
            }
        }
        unreachable!("Received reply for unknown peripheral #{addr}!");
    }
}
