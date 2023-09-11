use profirust::dp;
use profirust::fdl;
use profirust::phy;

// Encoder Parameters
const ENCODER_ADDRESS: u8 = 6;

// Bus Parameters
const BUS_DEVICE: &'static str = "/dev/ttyUSB0";
const BAUDRATE: profirust::Baudrate = profirust::Baudrate::B19200;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("FRABA 58XX Encoder Example");

    let mut dp_master = dp::DpMaster::new(vec![]);

    let encoder_options = dp::PeripheralOptions {
        ident_number: 0x4711,

        user_parameters: Some(&[
            0x00, 0x0a, 0x00, 0x00, 0x10, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ]),
        config: Some(&[0xf1]), // 2 word input&output

        ..Default::default()
    };
    let mut buffer_inputs = [0x00; 4];
    let mut buffer_outputs = [0x00; 4];
    let encoder_handle = dp_master.add(dp::Peripheral::new(
        ENCODER_ADDRESS,
        encoder_options,
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

    fdl_master.set_online();
    dp_master.enter_operate();
    loop {
        fdl_master.poll(profirust::time::Instant::now(), &mut phy, &mut dp_master);

        let cycle_completed = dp_master.cycle_completed();

        // Get mutable access the the peripheral here so we can interact with it.
        let encoder = dp_master.get_mut(encoder_handle);

        if encoder.is_running() && cycle_completed {
            let value = u32::from_be_bytes(encoder.pi_i().try_into().unwrap());
            println!("Encoder Counts: {:?}", value);
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
