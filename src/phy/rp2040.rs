use embedded_hal::digital::v2::OutputPin;
use rp2040_hal::uart;

use fugit::RateExtU32;
use rp2040_hal::Clock;

#[derive(Debug)]
enum PhyData<'a> {
    Rx {
        buffer: crate::phy::BufferHandle<'a>,
        length: usize,
    },
    Tx {
        buffer: crate::phy::BufferHandle<'a>,
        length: usize,
        cursor: usize,
    },
}

impl PhyData<'_> {
    pub fn is_rx(&self) -> bool {
        match self {
            PhyData::Rx { .. } => true,
            _ => false,
        }
    }

    pub fn is_tx(&self) -> bool {
        match self {
            PhyData::Tx { .. } => true,
            _ => false,
        }
    }

    pub fn make_rx(&mut self) {
        if let PhyData::Tx { buffer, .. } = self {
            let buffer = core::mem::replace(buffer, (&mut [][..]).into());
            *self = PhyData::Rx { buffer, length: 0 };
        }
    }
}

#[derive(Debug)]
pub struct Rp2040Phy<'a, UART, DIR> {
    uart: UART,
    dir_pin: DIR,
    data: PhyData<'a>,
}

impl<'a, D, P, DIR> Rp2040Phy<'a, uart::UartPeripheral<uart::Enabled, D, P>, DIR>
where
    D: uart::UartDevice,
    P: uart::ValidUartPinout<D>,
    DIR: OutputPin,
{
    pub fn new(
        uart: uart::UartPeripheral<uart::Disabled, D, P>,
        mut dir_pin: DIR,
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

        // Go into RX mode.
        dir_pin.set_low().ok().unwrap();

        Ok(Self {
            uart,
            dir_pin,
            data: PhyData::Rx {
                buffer: buffer.into(),
                length: 0,
            },
        })
    }
}

impl<'a, D, P, DIR> crate::phy::ProfibusPhy
    for Rp2040Phy<'a, uart::UartPeripheral<uart::Enabled, D, P>, DIR>
where
    D: uart::UartDevice,
    P: uart::ValidUartPinout<D>,
    DIR: OutputPin,
{
    fn is_transmitting(&mut self) -> bool {
        if let PhyData::Tx {
            buffer,
            length,
            cursor,
        } = &mut self.data
        {
            if length != cursor {
                let pending = &buffer[*cursor..*length];
                let written = match self.uart.write_raw(pending) {
                    Ok(b) => pending.len() - b.len(),
                    Err(nb::Error::WouldBlock) => 0,
                    Err(nb::Error::Other(_)) => unreachable!(),
                };
                debug_assert!(written <= *length - *cursor);
                *cursor += written;
                true
            } else {
                // TODO: Upstream HAL does not yet provide access to this field.
                let busy = unsafe {
                    (*rp2040_hal::pac::UART0::PTR)
                        .uartfr
                        .read()
                        .busy()
                        .bit_is_set()
                };
                if !busy {
                    self.data.make_rx();
                    self.dir_pin.set_low().ok().unwrap();
                }
                busy
            }
        } else {
            false
        }
    }

    fn transmit_data<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> (usize, R),
    {
        match &mut self.data {
            PhyData::Tx { .. } => panic!("transmit_data() while already transmitting!"),
            PhyData::Rx {
                buffer,
                length: receive_length,
            } => {
                if *receive_length != 0 {
                    log::warn!(
                        "{} bytes in the receive buffer and we go into transmission?",
                        receive_length
                    );
                }
                let (length, res) = f(&mut buffer[..]);
                if length == 0 {
                    // Don't transmit anything.
                    return res;
                }

                // TODO: timing considerations?  for now, let's only set the pin here and hope that
                // enough time passes before the next `is_transmitting()` call happens.
                self.dir_pin.set_high().ok().unwrap();

                // TODO: delay for the transmitter (roughly Tset)
                cortex_m::asm::delay(13020);

                let cursor = match self.uart.write_raw(&buffer[..length]) {
                    Ok(b) => length - b.len(),
                    Err(nb::Error::WouldBlock) => 0,
                    Err(nb::Error::Other(_)) => unreachable!(),
                };
                debug_assert!(cursor <= length);
                let buffer = core::mem::replace(buffer, (&mut [][..]).into());
                self.data = PhyData::Tx {
                    buffer,
                    length,
                    cursor,
                };
                res
            }
        }
    }

    fn receive_data<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> (usize, R),
    {
        match &mut self.data {
            PhyData::Tx { .. } => panic!("receive_data() while transmitting!"),
            PhyData::Rx { buffer, length } => {
                *length += match self.uart.read_raw(&mut buffer[*length..]) {
                    Ok(l) => l,
                    Err(nb::Error::WouldBlock) => 0,
                    Err(nb::Error::Other(_)) => {
                        // TODO: handle uart errors
                        0
                    }
                };
                debug_assert!(*length <= buffer.len());
                let (drop, res) = f(&buffer[..*length]);
                match drop {
                    0 => (),
                    d if d == *length => *length = 0,
                    d => todo!("drop partial receive buffer ({} bytes of {})", d, *length),
                }
                res
            }
        }
    }
}
