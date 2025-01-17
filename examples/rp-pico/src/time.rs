use crate::hal;

static mut TIMER: Option<hal::Timer> = None;

/// # Safety
/// This function must be called in exclusive execution context (e.g. critical section) and no call
/// to [`now()`] must run concurrently.
pub unsafe fn init(timer: hal::Timer) {
    unsafe {
        TIMER = Some(timer);
    }
}

pub fn now() -> Option<profirust::time::Instant> {
    // SAFETY: This is safe because the only time mutability could exist for TIMER is in the init
    // function which is guaranteed to be called during initialization where no concurrent call to
    // now() would be possible.
    #[allow(static_mut_refs)]
    let timer = unsafe { TIMER.as_ref() };
    timer.map(|timer| {
        profirust::time::Instant::from_micros(i64::try_from(timer.get_counter().ticks()).unwrap())
    })
}
