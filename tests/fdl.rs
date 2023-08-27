use profirust::fdl;
use profirust::phy;
use std::sync::atomic;

#[test]
fn two_masters_and_their_tokens() {
    let timestamp = std::sync::Arc::new(atomic::AtomicI64::new(0));
    let active_master = std::sync::Arc::new(atomic::AtomicU8::new(0));
    env_logger::builder()
        .is_test(true)
        .format({
            let timestamp = timestamp.clone();
            let active_master = active_master.clone();
            move |buf, record| {
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
                    "[{:16} {} {:32} M#{}] {}",
                    timestamp.load(atomic::Ordering::Relaxed),
                    level_str,
                    record.module_path().unwrap_or(""),
                    active_master.load(atomic::Ordering::Relaxed),
                    record.args(),
                )
            }
        })
        .init();

    let baud = profirust::Baudrate::B19200;

    let mut phy1 = phy::SimulatorPhy::new(baud, "phy1");
    let mut phy2 = phy1.duplicate("phy2");

    let mut per1 = fdl::PeripheralSet::new(vec![]);
    let mut master1 = fdl::FdlMaster::new(fdl::Parameters {
        address: 2,
        baudrate: baud,
        highest_station_address: 16,
        slot_bits: 300,
        ..Default::default()
    });
    let mut per2 = fdl::PeripheralSet::new(vec![]);
    let mut master2 = fdl::FdlMaster::new(fdl::Parameters {
        address: 7,
        baudrate: baud,
        highest_station_address: 16,
        slot_bits: 300,
        ..Default::default()
    });

    active_master.store(2, atomic::Ordering::Relaxed);
    master1.enter_operate();

    active_master.store(7, atomic::Ordering::Relaxed);
    master2.enter_operate();

    let start = profirust::time::Instant::ZERO;
    let mut now = start;
    while (now - start) < profirust::time::Duration::from_millis(800) {
        timestamp.store(now.total_micros() as i64, atomic::Ordering::Relaxed);
        phy1.set_bus_time(now);

        active_master.store(2, atomic::Ordering::Relaxed);
        master1.poll(now, &mut phy1, &mut per1);

        active_master.store(7, atomic::Ordering::Relaxed);
        master2.poll(now, &mut phy2, &mut per2);

        now += profirust::time::Duration::from_micros(100);
    }

    assert!(master1.is_in_ring());
    assert!(master2.is_in_ring());

    for i in 0..24 {
        assert_eq!(
            master1.check_address_live(i),
            i == 2 || i == 7,
            "wrong liveness of address {i} reported by master1(#2)"
        );
        assert_eq!(
            master2.check_address_live(i),
            i == 2 || i == 7,
            "wrong liveness of address {i} reported by master2(#7)"
        );
    }
}
