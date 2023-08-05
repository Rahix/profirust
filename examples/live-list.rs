use profirust::fdl;
use profirust::phy;

// Bus Parameters
const BUS_DEVICE: &'static str = "/dev/ttyUSB0";
const BAUDRATE: profirust::Baudrate = profirust::Baudrate::B19200;

fn main() {
    env_logger::init();

    println!("PROFIBUS Live List:");

    let mut peripherals = fdl::PeripheralSet::new(vec![]);

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

    let mut i = 0u64;

    master.enter_operate();
    loop {
        master.poll(profirust::time::Instant::now(), &mut phy, &mut peripherals);

        if i % 100 == 0 {
            let live_list: Vec<_> = master
                .iter_live_stations()
                .map(|addr| addr.to_string())
                .collect();
            println!("Live Addresses: {}", live_list.join(", "));
        }

        i += 1;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
