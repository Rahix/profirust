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
    #[inline(always)]
    pub fn address(&self) -> u8 {
        self.address
    }

    #[inline(always)]
    pub fn pi_i(&self) -> &[u8] {
        &self.pi_i
    }

    #[inline(always)]
    pub fn pi_q(&self) -> &[u8] {
        &self.pi_q
    }

    #[inline(always)]
    pub fn pi_q_mut(&mut self) -> &mut [u8] {
        &mut self.pi_q
    }
}

impl<'a> Peripheral<'a> {
    fn try_communicate(
        &mut self,
        now: crate::time::Instant,
        master: &crate::fdl::FdlMaster,
    ) -> Option<()> {
        if !master.check_address_live(self.address) {
            self.state = PeripheralState::Offline;
            return None;
        } else if self.state == PeripheralState::Offline {
            // Live but we're still "offline" => go to "reset" state
            self.state = PeripheralState::Reset;
        }

        match self.state {
            PeripheralState::Reset => todo!(),
            PeripheralState::WaitForParam => todo!(),
            PeripheralState::WaitForConfig => todo!(),
            PeripheralState::DataExchange => todo!(),
            PeripheralState::Offline => unreachable!(),
        }
    }
}
