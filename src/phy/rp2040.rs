use rp2040_hal::uart;

use fugit::RateExtU32;
use rp2040_hal::Clock;

#[derive(Debug)]
pub struct Rp2040Phy<'a, UART> {
    uart: UART,
    buf: crate::phy::BufferHandle<'a>,
}

impl<'a, D, P> Rp2040Phy<'a, uart::UartPeripheral<uart::Enabled, D, P>>
where
    D: uart::UartDevice,
    P: uart::ValidUartPinout<D>,
{
    pub fn new(
        uart: uart::UartPeripheral<uart::Disabled, D, P>,
        per_clock: &rp2040_hal::clocks::PeripheralClock,
        buffer: impl Into<crate::phy::BufferHandle<'a>>,
        baudrate: crate::Baudrate,
    ) -> Result<Self, uart::Error> {
        let uart = uart.enable(
            uart::UartConfig::new(
                u32::try_from(baudrate.to_rate()).unwrap().Hz(),
                uart::DataBits::Eight,
                Some(uart::Parity::Even),
                uart::StopBits::One,
            ),
            per_clock.freq(),
        )?;

        Ok(Self {
            uart,
            buf: buffer.into(),
        })
    }
}

impl<'a, D, P> crate::phy::ProfibusPhy for Rp2040Phy<'a, uart::UartPeripheral<uart::Enabled, D, P>>
where
    D: uart::UartDevice,
    P: uart::ValidUartPinout<D>,
{
    fn is_transmitting(&mut self) -> bool {
        todo!()
    }

    fn transmit_data<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> (usize, R),
    {
        todo!()
    }

    fn receive_data<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> (usize, R),
    {
        todo!()
    }
}
