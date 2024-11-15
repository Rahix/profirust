use std::borrow::Cow;
use std::io;

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
            let buffer = std::mem::replace(buffer, [].into());
            *self = PhyData::Rx { buffer, length: 0 };
        }
    }
}

pub struct SerialPortPhy {
    port: Box<dyn serialport::SerialPort>,
    data: PhyData<'static>,
}

impl SerialPortPhy {
    pub fn new<'a, P: Into<Cow<'a, str>>>(serial_port: P, baudrate: crate::Baudrate) -> Self {
        Self::new_inner(serial_port.into(), baudrate)
    }

    fn new_inner(serial_port: Cow<'_, str>, baudrate: crate::Baudrate) -> Self {
        let port = serialport::new(serial_port, u32::try_from(baudrate.to_rate()).unwrap())
            .data_bits(serialport::DataBits::Eight)
            .flow_control(serialport::FlowControl::None)
            .parity(serialport::Parity::Even)
            .stop_bits(serialport::StopBits::One)
            .open()
            .unwrap();

        assert_eq!(
            u64::from(port.baud_rate().unwrap()),
            baudrate.to_rate(),
            "baudrate not configured correctly"
        );

        let buffer = crate::phy::BufferHandle::from(vec![0u8; 512]);

        Self {
            port,
            data: PhyData::Rx { buffer, length: 0 },
        }
    }

    fn write(port: &mut dyn serialport::SerialPort, buffer: &[u8]) -> io::Result<usize> {
        // TODO: Technically we need to ensure this never blocks
        port.write(buffer)
    }

    fn get_output_queue(&mut self) -> io::Result<usize> {
        Ok(usize::try_from(self.port.bytes_to_write().unwrap()).unwrap())
    }

    fn read(port: &mut dyn serialport::SerialPort, buffer: &mut [u8]) -> io::Result<usize> {
        let bytes_to_read = port.bytes_to_read().unwrap();
        if bytes_to_read == 0 {
            Ok(0)
        } else {
            // TODO: Check that this will always return the available bytes immediately
            port.read(buffer)
        }
    }
}

impl crate::phy::ProfibusPhy for SerialPortPhy {
    fn poll_transmission(&mut self, _now: crate::time::Instant) -> bool {
        if let PhyData::Tx {
            buffer,
            length,
            cursor,
        } = &mut self.data
        {
            if length != cursor {
                // Need to submit more data.
                let written = Self::write(&mut *self.port, &buffer[*cursor..*length]).unwrap();
                debug_assert!(written <= *length - *cursor);
                *cursor += written;
                true
            } else {
                // Everything was submitted already.
                let queued = self.get_output_queue().unwrap();
                if queued == 0 {
                    // All data was sent.
                    self.data.make_rx();
                    false
                } else {
                    // Still sending.
                    true
                }
            }
        } else {
            false
        }
    }

    fn transmit_data<F, R>(&mut self, _now: crate::time::Instant, f: F) -> R
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
                let cursor = Self::write(&mut *self.port, &buffer[..length]).unwrap();
                debug_assert!(cursor <= length);
                let buffer = std::mem::replace(buffer, [].into());
                self.data = PhyData::Tx {
                    buffer,
                    length,
                    cursor,
                };
                res
            }
        }
    }

    fn receive_data<F, R>(&mut self, _now: crate::time::Instant, f: F) -> R
    where
        F: FnOnce(&[u8]) -> (usize, R),
    {
        match &mut self.data {
            PhyData::Tx { .. } => panic!("receive_data() while transmitting!"),
            PhyData::Rx { buffer, length } => {
                *length += Self::read(&mut *self.port, &mut buffer[*length..]).unwrap();
                debug_assert!(*length <= buffer.len());
                let (drop, res) = f(&buffer[..*length]);
                match drop {
                    0 => (),
                    d if d == *length => *length = 0,
                    d => {
                        assert!(d < *length);
                        for i in 0..(*length - d) {
                            buffer[i] = buffer[i + d];
                        }
                        *length -= d;
                    }
                }
                res
            }
        }
    }
}
