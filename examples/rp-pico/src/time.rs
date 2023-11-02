use crate::hal;

static mut TIMER: Option<hal::Timer> = None;

pub fn init(timer: hal::Timer) {
    unsafe {
        TIMER = Some(timer);
    }
}

pub fn now() -> Option<profirust::time::Instant> {
    let timer = unsafe { TIMER.as_ref() };
    timer.map(|timer| {
        profirust::time::Instant::from_micros(i64::try_from(timer.get_counter().ticks()).unwrap())
    })
}
