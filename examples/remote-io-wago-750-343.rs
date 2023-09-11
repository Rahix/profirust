use profirust::dp;
use profirust::fdl;
use profirust::phy;

// I/O Station Parameters
const IO_STATION_ADDRESS: u8 = 8;

// Bus Parameters
const BUS_DEVICE: &'static str = "/dev/ttyUSB0";
const BAUDRATE: profirust::Baudrate = profirust::Baudrate::B19200;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("WAGO 750-343 Remote I/O Station Example");

    let mut dp_master = dp::DpMaster::new(vec![]);

    let remoteio_options = dp::PeripheralOptions {
        ident_number: 0xb757,

        watchdog: Some(profirust::time::Duration::from_secs(2)),

        user_parameters: Some(&[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0xc3, 0x00, 0x00, 0x00, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x00, 0x80, 0x2b, 0x00, 0x21, 0x01, 0x00, 0x21, 0x01, 0x00,
            0x21, 0x01, 0x00, 0x21, 0x01, 0x00, 0x21, 0x01, 0x00, 0x21, 0x01, 0x00, 0x21, 0x01,
            0x00, 0x21, 0x01, 0x00, 0x21, 0x01, 0x00, 0x21, 0x01, 0x00, 0x21, 0x02, 0x01, 0x21,
            0x02, 0x02, 0x21, 0x02, 0x04, 0x21, 0x02, 0x08, 0x21, 0x02, 0x00, 0x21, 0x02, 0x00,
            0x21, 0x02, 0x00,
        ]),
        config: Some(&[
            0x00, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x20, 0x20, 0x20,
            0x20, 0x20, 0x20, 0x20,
        ]),

        ..Default::default()
    };

    let mut buffer_inputs = [0x00; 10];
    let mut buffer_outputs = [0x00; 7];
    let io_handle = dp_master.peripherals.add(dp::Peripheral::new(
        IO_STATION_ADDRESS,
        remoteio_options,
        &mut buffer_inputs,
        &mut buffer_outputs,
    ));

    let mut fdl_master = fdl::FdlMaster::new(fdl::Parameters {
        // Address of this master, i.e. ourselves
        address: 0x02,
        // Baudrate for bus communication
        baudrate: BAUDRATE,
        // We use a rather large T_sl time because USB-RS485 converters can induce large delays at
        // times.
        slot_bits: 1920,
        token_rotation_bits: 20000,
        ..Default::default()
    });

    println!("Connecting to the bus...");
    let mut phy = phy::LinuxRs485Phy::new(BUS_DEVICE, fdl_master.parameters().baudrate);

    let start = profirust::time::Instant::now();

    fdl_master.set_online();
    dp_master.state.enter_operate();
    loop {
        let now = profirust::time::Instant::now();
        fdl_master.poll(now, &mut phy, &mut dp_master);

        // Get mutable access the the peripheral here so we can interact with it.
        let remoteio = dp_master.peripherals.get_mut(io_handle);

        if remoteio.is_running() && dp_master.state.cycle_completed() {
            println!("Inputs: {:?}", remoteio.pi_i());

            // Set outputs according to our best intentions
            let elapsed = (now - start).total_millis();
            let i = usize::try_from(elapsed / 100).unwrap() % (remoteio.pi_q().len() * 4);
            let pi_q = remoteio.pi_q_mut();
            pi_q.fill(0x00);
            pi_q[i / 4] |= 1 << (i % 4);
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
