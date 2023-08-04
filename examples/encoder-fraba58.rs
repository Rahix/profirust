use profirust::fdl;
use profirust::peripheral;
use profirust::phy;

// Encoder Parameters
const ENCODER_ADDRESS: u8 = 6;

// Bus Parameters
const BUS_DEVICE: &'static str = "/dev/ttyUSB0";
const BAUDRATE: profirust::Baudrate = profirust::Baudrate::B19200;

fn main() {
    env_logger::init();

    println!("FRABA 58XX Encoder Example");

    let mut peripherals = fdl::PeripheralSet::new(vec![]);

    let encoder_options = peripheral::PeripheralOptions {
        ident_number: 0x4711,

        user_parameters: Some(&[
            0x00, 0x0a, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ]),
        config: Some(&[0xf1]), // 2 word input&output

        ..Default::default()
    };
    let mut buffer_inputs = [0x00; 4];
    let mut buffer_outputs = [0x00; 4];
    let encoder_handle = peripherals.add(peripheral::Peripheral::new(
        ENCODER_ADDRESS,
        encoder_options,
        &mut buffer_inputs,
        &mut buffer_outputs,
    ));

    let mut master = fdl::FdlMaster::new(fdl::Parameters {
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
    let mut phy = phy::LinuxRs485Phy::new(BUS_DEVICE, master.parameters().baudrate);

    enum State {
        WaitingForRing,
        WaitingForDevice,
        WaitingForDeviceInit,
        Running,
    }
    let mut state = State::WaitingForRing;

    loop {
        master.poll(profirust::time::Instant::now(), &mut phy, &mut peripherals);

        // Get mutable access the the peripheral here so we can interact with it.
        let encoder = peripherals.get_mut(encoder_handle);

        match state {
            State::WaitingForRing if master.is_in_ring() => {
                println!("Entered the token ring!");
                state = State::WaitingForDevice;
            }
            State::WaitingForRing => (),
            _ if !master.is_in_ring() => {
                println!("Master dropped out of the token ring!");
                state = State::WaitingForRing;
            }

            State::WaitingForDevice if encoder.is_live() => {
                println!("Device at address {} is responding!", encoder.address());
                state = State::WaitingForDeviceInit;
            }
            State::WaitingForDevice => (),
            _ if !encoder.is_live() => {
                println!(
                    "Device at address {} no longer responding!  Waiting for it again...",
                    encoder.address()
                );
                state = State::WaitingForDevice;
            }

            State::WaitingForDeviceInit if encoder.is_running() => {
                println!("Device configured successfully!");
                state = State::Running;
            }
            State::WaitingForDeviceInit => (),
            _ if !encoder.is_running() => {
                println!("Cyclic data exchange stopped for some reason!");
                state = State::WaitingForDeviceInit;
            }

            State::Running => {
                let value = u32::from_be_bytes(encoder.pi_i().try_into().unwrap());
                println!("Encoder Counts: {:?}", value);
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
