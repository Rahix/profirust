use std::sync;

#[derive(Debug)]
pub struct TestBusPhy {
    bus: sync::Arc<sync::Mutex<bus::Bus<Vec<u8>>>>,
    rx: bus::BusReader<Vec<u8>>,
}

impl Clone for TestBusPhy {
    fn clone(&self) -> Self {
        let rx = self
            .bus
            .lock()
            .expect("failed locking testbus mutex")
            .add_rx();
        Self {
            bus: self.bus.clone(),
            rx,
        }
    }
}

impl TestBusPhy {
    pub fn new() -> Self {
        let mut bus = bus::Bus::new(256);
        let rx = bus.add_rx();
        Self {
            bus: sync::Arc::new(sync::Mutex::new(bus)),
            rx,
        }
    }
}

impl crate::phy::ProfibusPhy for TestBusPhy {
    fn is_transmitting(&mut self) -> bool {
        // For simplicity, we don't simulate transmission taking any time...
        false
    }

    fn transmit_data<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> (usize, R),
    {
        // We can be sure that in the real world, no received messages would exist here, so clear
        // the buffer.
        while let Ok(_) = self.rx.try_recv() {}

        let mut buffer = vec![0u8; 256];
        let (length, res) = f(&mut buffer);
        buffer.truncate(length);
        self.bus
            .lock()
            .expect("failed locking testbus mutex")
            .broadcast(buffer);

        // And immediately receive the message again to ensure we're not reading it.  Yes, this is
        // a racy thing to do...
        self.rx.recv().unwrap();

        res
    }

    fn receive_data<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> (usize, R),
    {
        if let Ok(buffer) = self.rx.try_recv() {
            let (drop, res) = f(&buffer);
            assert!(drop == buffer.len());
            res
        } else {
            f(&[]).1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phy::ProfibusPhy;

    #[test]
    fn simple_send_recv() {
        let mut phy1 = TestBusPhy::new();
        let mut phy2 = phy1.clone();
        let mut phy3 = phy1.clone();

        let telegram = crate::fdl::DataTelegram::fdl_status(4, 2);
        phy1.transmit_telegram(telegram.clone().into());
        let res = phy2.receive_telegram().unwrap();
        assert_eq!(res, telegram.clone().into());

        let res = phy3.receive_telegram().unwrap();
        assert_eq!(res, telegram.into());
    }
}
