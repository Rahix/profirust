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

pub struct Peripheral<'a> {
    /// Station address of this peripheral (slave)
    address: u8,
    /// Current state of this peripheral
    state: PeripheralState,
    /// Process Image of Inputs
    pi_i: &'a mut [u8],
    /// Process Image of Outputs
    pi_q: &'a mut [u8],
}

impl<'a> Peripheral<'a> {
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
                        fc: crate::fdl::FunctionCode::Request {
                            fcv: false,
                            fcb: false,
                            req: crate::fdl::RequestType::SrdLow,
                        },
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
        log::debug!("I got a {telegram:?}");
    }
}
