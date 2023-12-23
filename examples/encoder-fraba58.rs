use profirust::dp;
use profirust::fdl;
use profirust::phy;

// Encoder Parameters
const ENCODER_ADDRESS: u8 = 6;

// Bus Parameters
const MASTER_ADDRESS: u8 = 3;
const BUS_DEVICE: &'static str = "/dev/ttyUSB0";
const BAUDRATE: profirust::Baudrate = profirust::Baudrate::B19200;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_micros()
        .init();

    println!("FRABA 58XX Encoder Example");

    let mut dp_master = dp::DpMaster::new(vec![]);

    // Options generated by `gsdtool` using "FRAB4711.gsd"
    let options = profirust::dp::PeripheralOptions {
        // "FRABA Encoder" by "FRABA"
        ident_number: 0x4711,

        // Global Parameters:
        //   (none)
        //
        // Selected Modules:
        //   [0] Class 2 Multiturn
        //       - Code sequence.................: Increasing clockwise (0)
        //       - Class 2 functionality.........: Enable
        //       - Scaling function control......: Enable
        //       - Measuring units per revolution: 4096
        //       - Total measuring range (high)..: 256
        //       - Total measuring range (low)...: 0
        user_parameters: Some(&[
            0x00, 0x0a, 0x00, 0x00, 0x10, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ]),
        config: Some(&[0xf1]),

        // Set max_tsdr depending on baudrate and assert
        // that a supported baudrate is used.
        max_tsdr: match BAUDRATE {
            profirust::Baudrate::B9600 => 60,
            profirust::Baudrate::B19200 => 60,
            profirust::Baudrate::B93750 => 60,
            profirust::Baudrate::B187500 => 60,
            profirust::Baudrate::B500000 => 100,
            b => panic!("Peripheral \"FRABA Encoder\" does not support baudrate {b:?}!"),
        },

        fail_safe: false,
        ..Default::default()
    };
    let mut buffer_inputs = [0u8; 4];
    let mut buffer_outputs = [0u8; 4];
    let mut buffer_diagnostics = [0u8; 57];
    let encoder_handle = dp_master.add(
        dp::Peripheral::new(
            ENCODER_ADDRESS,
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
            .build_verified(&dp_master),
    );

    println!("Connecting to the bus...");
    let mut phy = phy::LinuxRs485Phy::new(BUS_DEVICE, fdl_master.parameters().baudrate);

    fdl_master.set_online();
    dp_master.enter_operate();
    loop {
        let now = profirust::time::Instant::now();
        let events = fdl_master.poll(now, &mut phy, &mut dp_master);

        // Get mutable access the the peripheral here so we can interact with it.
        let encoder = dp_master.get_mut(encoder_handle);

        if encoder.is_running() && events.cycle_completed {
            let value = u32::from_be_bytes(encoder.pi_i().try_into().unwrap());
            println!("Encoder Counts: {:?}", value);
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
