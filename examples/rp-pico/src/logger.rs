use core::fmt::Write;
use cortex_m::interrupt;

struct RingBuffer {
    buffer: heapless::Deque<u8, 4096>,
}

struct RingBufferLogger {
    buffer: interrupt::Mutex<core::cell::RefCell<RingBuffer>>,
}

impl log::Log for RingBufferLogger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        let timestamp = crate::time::now().unwrap_or(profirust::time::Instant::ZERO);
        let color = match record.level() {
            log::Level::Error => "\x1B[31m",
            log::Level::Warn => "\x1B[1m",
            log::Level::Info => "",
            log::Level::Debug | log::Level::Trace => "\x1B[2m",
        };
        cortex_m::interrupt::free(|cs| {
            let mut buffer = self.buffer.borrow(cs).borrow_mut();
            if let Some(module_path) = record.module_path() {
                let _ = write!(
                    &mut buffer,
                    "\x1B[32m[{:5}.{:06}] \x1B[33m{}\x1B[0m: {}{}\x1B[0m\r\n",
                    timestamp.secs(),
                    timestamp.micros(),
                    module_path.trim_start_matches("vlab_ethernet_bridge_firmware::"),
                    color,
                    record.args()
                );
            } else {
                let _ = write!(
                    &mut buffer,
                    "\x1B[32m[{:12}] {}{}\r\n",
                    timestamp,
                    color,
                    record.args()
                );
            }
        })
    }

    fn flush(&self) {}
}

impl core::fmt::Write for RingBuffer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        if self.buffer.len() + s.len() > self.buffer.capacity() {
            for _ in 0..(s.len() - (self.buffer.capacity() - self.buffer.len())) {
                let _ = self.buffer.pop_front();
            }

            // After making enough space for the incoming log message, continue until we find a
            // newline to ensure we don't keep a cut-off message at the front of the buffer.
            loop {
                match self.buffer.pop_front() {
                    Some(0x0a) => break,
                    None => break,
                    _ => (),
                }
            }
        }
        for b in s.as_bytes() {
            let _ = self.buffer.push_back(*b);
        }
        Ok(())
    }
}

static LOGGER: RingBufferLogger = RingBufferLogger {
    buffer: interrupt::Mutex::new(core::cell::RefCell::new(RingBuffer {
        buffer: heapless::Deque::new(),
    })),
};

pub fn init() {
    unsafe {
        log::set_logger_racy(&LOGGER)
            .map(|()| log::set_max_level_racy(log::LevelFilter::Trace))
            .unwrap();
    }
}

pub fn drain<F: FnMut(&[u8]) -> usize>(mut f: F) {
    cortex_m::interrupt::free(|cs| {
        let mut buffer = LOGGER.buffer.borrow(cs).borrow_mut();
        let (slice1, slice2) = buffer.buffer.as_slices();
        let mut length = f(slice1);
        if length == slice1.len() {
            length += f(slice2);
        }
        for _ in 0..length {
            // TODO: Add safety assertions
            unsafe { buffer.buffer.pop_front_unchecked() };
        }
    })
}
