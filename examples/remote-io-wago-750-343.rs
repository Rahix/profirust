use profirust::dp;
use profirust::fdl;
use profirust::phy;

// I/O Station Parameters
const IO_STATION_ADDRESS: u8 = 8;

// Bus Parameters
const MASTER_ADDRESS: u8 = 3;
const BUS_DEVICE: &'static str = "/dev/ttyUSB0";
const BAUDRATE: profirust::Baudrate = profirust::Baudrate::B19200;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_micros()
        .init();

    println!("WAGO 750-343 Remote I/O Station Example");

    let mut dp_master = dp::DpMaster::new(vec![]);

    // Options generated by `gsdtool` using "wagob757.gsd"
    let options = profirust::dp::PeripheralOptions {
        // "WAGO 750-343" by "WAGO Kontakttechnik GmbH"
        ident_number: 0xb757,

        // Global Parameters:
        //   - DP-Watchdog-Base...............: 10 ms
        //   - Restart on K-Bus Failure.......: POWER ON RESET
        //   - Device Diagnosis...............: enabled
        //   - Process Data Representation....: MOTOROLA (MSB-LSB)
        //   - Response to PROFIBUS DP Failure: Substitude Values are switched
        //   - Response to K-Bus Failure......: PROFIBUS communication stops
        //
        // Selected Modules:
        //   [ 0] 750-343 No PI Channel
        //   [ 1] 750-402  4 DI/24 V DC/3.0 ms
        //   [ 2] 750-402  4 DI/24 V DC/3.0 ms
        //   [ 3] 750-402  4 DI/24 V DC/3.0 ms
        //   [ 4] 750-402  4 DI/24 V DC/3.0 ms
        //   [ 5] 750-402  4 DI/24 V DC/3.0 ms
        //   [ 6] 750-402  4 DI/24 V DC/3.0 ms
        //   [ 7] 750-402  4 DI/24 V DC/3.0 ms
        //   [ 8] 750-402  4 DI/24 V DC/3.0 ms
        //   [ 9] 750-402  4 DI/24 V DC/3.0 ms
        //   [10] 750-402  4 DI/24 V DC/3.0 ms
        //   [11] 750-504  4 DO/24 V DC/0.5 A
        //   [12] 750-504  4 DO/24 V DC/0.5 A
        //   [13] 750-504  4 DO/24 V DC/0.5 A
        //   [14] 750-504  4 DO/24 V DC/0.5 A
        //   [15] 750-504  4 DO/24 V DC/0.5 A
        //   [16] 750-504  4 DO/24 V DC/0.5 A
        //   [17] 750-504  4 DO/24 V DC/0.5 A
        user_parameters: Some(&[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0xc3, 0x00, 0x00, 0x00, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x00, 0x80, 0x2b, 0x00, 0x21, 0x01, 0x00, 0x21, 0x01, 0x00,
            0x21, 0x01, 0x00, 0x21, 0x01, 0x00, 0x21, 0x01, 0x00, 0x21, 0x01, 0x00, 0x21, 0x01,
            0x00, 0x21, 0x01, 0x00, 0x21, 0x01, 0x00, 0x21, 0x01, 0x00, 0x21, 0x02, 0x00, 0x21,
            0x02, 0x00, 0x21, 0x02, 0x00, 0x21, 0x02, 0x00, 0x21, 0x02, 0x00, 0x21, 0x02, 0x00,
            0x21, 0x02, 0x00,
        ]),
        config: Some(&[
            0x00, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x20, 0x20, 0x20,
            0x20, 0x20, 0x20, 0x20,
        ]),

        // Set max_tsdr depending on baudrate and assert
        // that a supported baudrate is used.
        max_tsdr: match BAUDRATE {
            profirust::Baudrate::B9600 => 60,
            profirust::Baudrate::B19200 => 60,
            profirust::Baudrate::B93750 => 60,
            profirust::Baudrate::B187500 => 60,
            profirust::Baudrate::B500000 => 100,
            b => panic!("Peripheral \"WAGO 750-343\" does not support baudrate {b:?}!"),
        },

        fail_safe: true,
        ..Default::default()
    };
    let mut buffer_inputs = [0u8; 10];
    let mut buffer_outputs = [0u8; 7];
    let mut buffer_diagnostics = [0u8; 64];
    let io_handle = dp_master.add(
        dp::Peripheral::new(
            IO_STATION_ADDRESS,
            options,
            &mut buffer_inputs,
            &mut buffer_outputs,
        )
        .with_diag_buffer(&mut buffer_diagnostics),
    );

    let mut fdl_master = fdl::FdlMaster::new(
        fdl::ParametersBuilder::new(MASTER_ADDRESS, BAUDRATE)
            // We use a rather large T_slot time because USB-RS485 converters can induce large delays at
            // times.
            .slot_bits(1920)
            .watchdog_timeout(profirust::time::Duration::from_secs(2))
            .build_verified(&dp_master),
    );

    println!("Connecting to the bus...");
    let mut phy = phy::LinuxRs485Phy::new(BUS_DEVICE, fdl_master.parameters().baudrate);

    let start = profirust::time::Instant::now();

    fdl_master.set_online();
    dp_master.enter_operate();
    loop {
        let now = profirust::time::Instant::now();
        let events = fdl_master.poll(now, &mut phy, &mut dp_master);

        // Get mutable access the the peripheral here so we can interact with it.
        let remoteio = dp_master.get_mut(io_handle);

        if remoteio.is_running() && events.cycle_completed {
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
