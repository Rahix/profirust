use std::sync::atomic;

static LOG_TIMESTAMP: atomic::AtomicI64 = atomic::AtomicI64::new(0);
static ACTIVE_ADDR: atomic::AtomicU8 = atomic::AtomicU8::new(0);

pub fn prepare_test_logger() {
    let _ = env_logger::builder()
        .is_test(true)
        .format(move |buf, record| {
            use std::io::Write;
            let level_str = match record.level() {
                log::Level::Error => "\x1b[31mERROR\x1b[0m",
                log::Level::Warn => "\x1b[33mWARN \x1b[0m",
                log::Level::Info => "\x1b[34mINFO \x1b[0m",
                log::Level::Debug => "\x1b[35mDEBUG\x1b[0m",
                log::Level::Trace => "\x1b[36mTRACE\x1b[0m",
            };
            writeln!(
                buf,
                "[{:16} {} {:32} #{}] {}",
                LOG_TIMESTAMP.load(atomic::Ordering::Relaxed),
                level_str,
                record.module_path().unwrap_or(""),
                ACTIVE_ADDR.load(atomic::Ordering::Relaxed),
                record.args(),
            )
        })
        .filter_level(log::LevelFilter::Trace)
        .try_init();

    set_log_timestamp(crate::time::Instant::ZERO);
}

pub fn set_log_timestamp(t: crate::time::Instant) {
    LOG_TIMESTAMP.store(t.total_millis(), atomic::Ordering::SeqCst);
}

pub fn set_active_addr(addr: u8) {
    ACTIVE_ADDR.store(addr, atomic::Ordering::SeqCst);
}
