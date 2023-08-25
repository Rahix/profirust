use profirust::fdl;
use profirust::phy;

#[test]
fn two_masters_and_their_tokens() {
    let _ = env_logger::try_init();

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

    master1.enter_operate();
    master2.enter_operate();

    let start = profirust::time::Instant::now();
    let mut now = start;
    while (now - start) < profirust::time::Duration::from_millis(800) {
        phy1.set_bus_time(now);

        log::trace!("M#2 ---");
        master1.poll(now, &mut phy1, &mut per1);
        log::trace!("M#7 ---");
        master2.poll(now, &mut phy2, &mut per2);

        now += profirust::time::Duration::from_millis(1);
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
