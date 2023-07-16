use profirust::fdl;
use profirust::phy;
use profirust::phy::ProfibusPhy;

#[ignore = "not connected"]
#[test]
fn fdl_test() {
    env_logger::init();

    let mut peripherals = fdl::PeripheralSet::new(vec![]);
    let mut _wago_station = peripherals.add(profirust::peripheral::Peripheral::new(36));

    let mut master = fdl::FdlMaster::new(fdl::Parameters {
        address: 0x02,
        baudrate: profirust::Baudrate::B19200,
        // We use a rather large Tsl time because the USB-RS485 converter is super slow at times.
        slot_bits: 1920,
        token_rotation_bits: 20000,
        ..Default::default()
    });

    log::debug!("{:#?}", master.parameters());
    log::debug!(
        "Lost token timeout: {:?} ms",
        master.parameters().token_lost_timeout().millis()
    );

    let mut phy = phy::LinuxRs485Phy::new("/dev/ttyUSB0");

    loop {
        master.poll(profirust::time::Instant::now(), &mut phy, &mut peripherals);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[ignore = "not connected"]
#[test]
fn phy_test() {
    env_logger::init();

    let mut phy = phy::LinuxRs485Phy::new("/dev/ttyUSB0");

    phy.transmit_data(|buffer| {
        buffer[..6].copy_from_slice(&[0x10, 0x22, 0x02, 0x49, 0x6D, 0x16]);
        (6, ())
    });

    while phy.is_transmitting() {
        phy.wait_transmit();
    }
    log::trace!("Sent request!");

    while phy.receive_data(|buffer| (0, buffer.len() < 5)) {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    let mut msg_buffer = [0u8; 256];
    let msg = phy.receive_data(|buffer| {
        assert!(buffer.len() >= 5);
        let msg = &mut msg_buffer[..buffer.len()];
        msg.copy_from_slice(buffer);
        (buffer.len(), msg)
    });
    log::debug!("{:?}", msg);

    let expected = [0x10, 0x02, 0x22, 0x00, 0x24, 0x16];
    assert_eq!(msg, expected);
}
