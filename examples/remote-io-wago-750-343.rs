use profirust::fdl;
use profirust::peripheral;
use profirust::phy;

// I/O Station Parameters
const IO_STATION_ADDRESS: u8 = 8;

// Bus Parameters
const BUS_DEVICE: &'static str = "/dev/ttyUSB0";
const BAUDRATE: profirust::Baudrate = profirust::Baudrate::B19200;

fn main() {
    env_logger::init();

    println!("WAGO 750-343 Remote I/O Station Example");

    let mut peripherals = fdl::PeripheralSet::new(vec![]);

    let remoteio_options = peripheral::PeripheralOptions {
        ident_number: 0xb757,

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
    let io_handle = peripherals.add(peripheral::Peripheral::new(
        IO_STATION_ADDRESS,
        remoteio_options,
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

    let start = profirust::time::Instant::now();
    loop {
        let now = profirust::time::Instant::now();
        master.poll(now, &mut phy, &mut peripherals);

        // Get mutable access the the peripheral here so we can interact with it.
        let remoteio = peripherals.get_mut(io_handle);

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

            State::WaitingForDevice if remoteio.is_live() => {
                println!("Device at address {} is responding!", remoteio.address());
                state = State::WaitingForDeviceInit;
            }
            State::WaitingForDevice => (),
            _ if !remoteio.is_live() => {
                println!(
                    "Device at address {} no longer responding!  Waiting for it again...",
                    remoteio.address()
                );
                state = State::WaitingForDevice;
            }

            State::WaitingForDeviceInit if remoteio.is_running() => {
                println!("Device configured successfully!");
                state = State::Running;
            }
            State::WaitingForDeviceInit => (),
            _ if !remoteio.is_running() => {
                println!("Cyclic data exchange stopped for some reason!");
                state = State::WaitingForDeviceInit;
            }

            State::Running => {
                println!("Inputs: {:?}", remoteio.pi_i());

                // Set outputs according to our best intentions
                let elapsed = (now - start).total_millis();
                let i = (elapsed / 100) % (remoteio.pi_q().len() as u64 * 4);
                let pi_q = remoteio.pi_q_mut();
                pi_q.fill(0x00);
                pi_q[(i / 4) as usize] |= 1 << (i % 4);
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
