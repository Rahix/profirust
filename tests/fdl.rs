use profirust::fdl;
use profirust::phy;

#[test]
fn two_masters_and_their_tokens() {
    let _ = env_logger::try_init();

    let mut phy1 = phy::TestBusPhy::new();
    let mut phy2 = phy1.clone();

    log::debug!("Say hello!");

    let mut master1 = fdl::FdlMaster::new(fdl::Parameters {
        address: 2,
        baudrate: fdl::Baudrate::B19200,
        highest_station_address: 16,
        ..Default::default()
    });
    let mut master2 = fdl::FdlMaster::new(fdl::Parameters {
        address: 7,
        baudrate: fdl::Baudrate::B19200,
        highest_station_address: 16,
        ..Default::default()
    });

    let mut i = 0;
    loop {
        log::trace!("I: {:8}", i);
        let now = profirust::time::Instant::now();

        master1.poll(now, &mut phy1);
        log::trace!("---");
        master2.poll(now, &mut phy2);
        std::thread::sleep(std::time::Duration::from_millis(1));
        i += 1;

        if i == 400 {
            break;
        }
    }
}
