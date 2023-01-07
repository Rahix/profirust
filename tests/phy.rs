use profirust::phy;
use profirust::phy::ProfibusPhy;

#[ignore = "not connected"]
#[test]
fn dummy_phy_test() {
    env_logger::init();

    let mut phy = phy::LinuxRs485Phy::new("/dev/ttyUSB0");

    let mut buffer: phy::BufferHandle = vec![0u8; 256].into();
    buffer[..6].copy_from_slice(&[0x10, 0x22, 0x02, 0x49, 0x6D, 0x16]);

    phy.schedule_tx(buffer, 6);
    let buffer = loop {
        if let Some(buf) = phy.poll_tx() {
            break buf;
        }
        // don't busy spin
        phy.wait_transmit();
    };
    log::trace!("Sent request!");

    phy.schedule_rx(buffer);

    while phy.peek_rx().len() < 5 {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    let (buffer, length) = phy.poll_rx();
    let msg = &buffer[..length];
    log::debug!("{:?}", msg);

    let expected = [0x10, 0x02, 0x22, 0x00, 0x24, 0x16];
    assert_eq!(msg, expected);
}
