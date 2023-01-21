use profirust::fdl;
use profirust::phy;

#[ignore = "not connected"]
#[test]
fn dummy_fdl_test() {
    env_logger::init();

    let mut master = fdl::FdlMaster::new(fdl::Parameters {
        address: 0x02,
        baudrate: fdl::Baudrate::B31250,
        ..Default::default()
    });

    log::debug!("{:#?}", master.parameters());
    log::debug!(
        "Lost token timeout: {:?} ms",
        master.parameters().token_lost_timeout().millis()
    );

    let mut phy = phy::LinuxRs485Phy::new("/dev/ttyUSB0");

    let mut i = 0;
    loop {
        log::trace!("I: {:8}", i);
        master.poll(profirust::time::Instant::now(), &mut phy);
        std::thread::sleep(std::time::Duration::from_millis(10));
        i += 1;
    }
}
