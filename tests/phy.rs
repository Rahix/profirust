use profirust::phy;
use profirust::phy::ProfibusPhy;

#[ignore = "not connected"]
#[test]
fn dummy_phy_test() {
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
