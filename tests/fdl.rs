use profirust::fdl;
use profirust::phy;

#[test]
fn two_masters_and_their_tokens() {
    let _ = env_logger::try_init();

    let mut phy1 = phy::TestBusPhy::new();
    let mut phy2 = phy1.clone();

    log::debug!("Say hello!");

    let mut per1 = fdl::PeripheralSet::new(vec![]);
    let mut master1 = fdl::FdlMaster::new(fdl::Parameters {
        address: 2,
        baudrate: profirust::Baudrate::B19200,
        highest_station_address: 16,
        ..Default::default()
    });
    let mut per2 = fdl::PeripheralSet::new(vec![]);
    let mut master2 = fdl::FdlMaster::new(fdl::Parameters {
        address: 7,
        baudrate: profirust::Baudrate::B19200,
        highest_station_address: 16,
        ..Default::default()
    });

    master1.enter_operate();
    master2.enter_operate();

    let mut i = 0;
    loop {
        log::trace!("I: {:8}", i);
        let now = profirust::time::Instant::now();

        log::trace!("M#2 ---");
        master1.poll(now, &mut phy1, &mut per1);
        log::trace!("M#7 ---");
        master2.poll(now, &mut phy2, &mut per2);
        std::thread::sleep(std::time::Duration::from_millis(1));
        i += 1;

        if i == 400 {
            break;
        }
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
