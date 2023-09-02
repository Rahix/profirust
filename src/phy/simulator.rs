use std::sync;

#[derive(Debug)]
struct CapturedTelegram {
    sender: &'static str,
    timestamp: crate::time::Instant,
    index: usize,
    length: usize,
}

#[derive(Debug)]
struct SimulatorBus {
    baudrate: crate::Baudrate,
    telegrams: Vec<CapturedTelegram>,
    stream: Vec<u8>,
    bus_time: crate::time::Instant,
    /// Which master is currently holding the token.  We use this to verify correct timing.
    token_master: Option<u8>,
}

impl SimulatorBus {
    pub fn new(baudrate: crate::Baudrate) -> Self {
        Self {
            baudrate,
            telegrams: Vec::new(),
            stream: Vec::new(),
            bus_time: crate::time::Instant::ZERO,
            token_master: None,
        }
    }

    pub fn current_cursor(&self) -> usize {
        if self.telegrams.len() == 0 {
            return 0;
        }

        let last_telegram = self.telegrams.last().unwrap();

        let last_telegram_tx_duration = self.bus_time - last_telegram.timestamp;
        let last_telegram_tx_bytes =
            (self.baudrate.time_to_bits(last_telegram_tx_duration) / 11) as usize;

        self.stream.len() - last_telegram.length + last_telegram_tx_bytes.min(last_telegram.length)
    }

    pub fn pending_bytes(&self, cursor: usize) -> &[u8] {
        &self.stream[cursor..self.current_cursor()]
    }

    pub fn is_active(&self) -> Option<&'static str> {
        if self.telegrams.len() == 0 {
            return None;
        }

        let last_telegram = self.telegrams.last().unwrap();

        let last_telegram_tx_duration = self.bus_time - last_telegram.timestamp;
        let last_telegram_tx_bytes =
            (self.baudrate.time_to_bits(last_telegram_tx_duration) / 11) as usize;

        if last_telegram_tx_bytes < last_telegram.length {
            Some(last_telegram.sender)
        } else {
            None
        }
    }

    pub fn enqueue_telegram(&mut self, name: &'static str, mut data: Vec<u8>) {
        if let Some(active_sender) = self.is_active() {
            panic!(
                "\"{}\" attempted transmission while \"{}\" is still sending!",
                name, active_sender
            );
        }

        let sa = if let Some(Ok(decoded)) = crate::fdl::Telegram::deserialize(&data) {
            match decoded {
                crate::fdl::Telegram::Token(crate::fdl::TokenTelegram { da, sa }) => {
                    self.token_master = Some(da);
                    Some(sa)
                }
                crate::fdl::Telegram::Data(crate::fdl::DataTelegram { h, pdu }) => Some(h.sa),
                _ => None,
            }
        } else {
            None
        };

        let min_delay = if sa == self.token_master {
            // Master must wait 33 bit synchronization pause before transmitting.
            33
        } else {
            // Peripherals must wait at least 11 bit minimum Tsdr before responding.
            11
        };

        // Ensure that at least 11 bit times were left between two consecutive transmissions.
        if let Some(last_telegram_and_pause) = self
            .telegrams
            .last()
            .map(|t| t.timestamp + self.baudrate.bits_to_time(t.length as u32 * 11 + min_delay))
        {
            if self.bus_time < last_telegram_and_pause {
                if sa == self.token_master {
                    panic!(
                        "\"{}\" did not leave synchronization pause before its transmission.",
                        name
                    );
                } else {
                    log::error!(
                        "\"{}\" did not leave minimum Tsdr time before its transmission.",
                        name
                    );
                }
            }
        }

        if let Some(Ok(decoded)) = crate::fdl::Telegram::deserialize(&data) {
            log::trace!("{:8} {}: {:?}", self.bus_time.total_micros(), name, decoded);
        } else {
            let data_fmt = data
                .iter()
                .map(|b| format!("0x{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");
            log::trace!("{:8} {}: {}", self.bus_time.total_micros(), name, data_fmt);
        }

        let telegram = CapturedTelegram {
            sender: name,
            timestamp: self.bus_time,
            index: self.stream.len(),
            length: data.len(),
        };
        self.stream.append(&mut data);
        self.telegrams.push(telegram);
    }

    pub fn print_log(&self) {
        for t in &self.telegrams {
            print!("{:16} {:>12}:", t.timestamp.total_micros(), t.sender);
            for b in &self.stream[t.index..t.index + t.length] {
                print!(" 0x{:02x}", b);
            }
            println!();
        }
    }

    // phy needs to find out what data is still pending for it
    // phy needs to find out whether it is still transmitting
    // phy needs to be able to submit new data
    //
    // bytes should become available with correct timing (i.e. no artificial framing)
    // bus should immediately panic on collision (at least for now)
}

#[derive(Debug)]
pub struct SimulatorPhy {
    bus: sync::Arc<sync::Mutex<SimulatorBus>>,
    cursor: usize,
    name: &'static str,
}

impl SimulatorPhy {
    pub fn new(baudrate: crate::Baudrate, name: &'static str) -> Self {
        Self {
            bus: sync::Arc::new(sync::Mutex::new(SimulatorBus::new(baudrate))),
            cursor: 0,
            name,
        }
    }

    pub fn duplicate(&self, name: &'static str) -> Self {
        Self {
            bus: self.bus.clone(),
            cursor: 0,
            name,
        }
    }

    pub fn set_bus_time(&self, time: crate::time::Instant) {
        self.bus.lock().unwrap().bus_time = time;
    }

    pub fn advance_bus_time(&self, dur: crate::time::Duration) {
        self.bus.lock().unwrap().bus_time += dur;
    }

    pub fn print_bus_log(&self) {
        self.bus.lock().unwrap().print_log();
    }
}

impl crate::phy::ProfibusPhy for SimulatorPhy {
    fn is_transmitting(&mut self) -> bool {
        let bus = self.bus.lock().unwrap();
        bus.is_active() == Some(self.name)
    }

    fn transmit_data<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> (usize, R),
    {
        let mut bus = self.bus.lock().unwrap();

        let mut buffer = vec![0u8; 256];
        let (length, res) = f(&mut buffer);
        buffer.truncate(length);

        bus.enqueue_telegram(self.name, buffer);
        self.cursor += length;

        res
    }

    fn receive_data<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> (usize, R),
    {
        if self.is_transmitting() {
            panic!(
                "\"{}\" attempted to receive while it was still transmitting!",
                self.name
            );
        }

        let bus = self.bus.lock().unwrap();
        let pending = bus.pending_bytes(self.cursor);

        let (drop, res) = f(pending);
        assert!(
            drop <= pending.len(),
            "\"{}\" attempted to drop more pending bytes than it has!",
            self.name,
        );
        self.cursor += drop;

        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phy::ProfibusPhy;

    #[test]
    fn send_and_receive() {
        let mut phy1 = SimulatorPhy::new(crate::Baudrate::B19200, "phy1");
        let mut phy2 = phy1.duplicate("phy2");

        let data = &[0xde, 0xad, 0xbe, 0xef, 0x12, 0x34];
        phy1.transmit_data(|buf| {
            buf[..data.len()].copy_from_slice(data);
            (data.len(), ())
        });

        phy2.receive_data(|buf| {
            assert_eq!(buf.len(), 0);
            (0, ())
        });

        phy1.advance_bus_time(crate::time::Duration::from_millis(100));

        phy2.receive_data(|buf| {
            assert_eq!(buf, data);
            (4, ())
        });
        phy2.receive_data(|buf| {
            assert_eq!(buf.len(), data.len() - 4);
            (buf.len(), ())
        });

        let data = &[0xc0, 0xff, 0xee];
        phy2.transmit_data(|buf| {
            buf[..data.len()].copy_from_slice(data);
            (data.len(), ())
        });

        phy1.advance_bus_time(crate::time::Duration::from_millis(100));

        phy1.receive_data(|buf| {
            assert_eq!(buf, data);
            (buf.len(), ())
        });

        phy1.print_bus_log();
    }
}
