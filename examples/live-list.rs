use profirust::fdl;
use profirust::phy;

// Bus Parameters
const MASTER_ADDRESS: u8 = 3;
const BUS_DEVICE: &'static str = "/dev/ttyUSB0";
const BAUDRATE: profirust::Baudrate = profirust::Baudrate::B500000;

fn main() {
    env_logger::init();

    println!("PROFIBUS Live List:");

    let mut fdl_master = fdl::FdlMaster::new(
        fdl::ParametersBuilder::new(MASTER_ADDRESS, BAUDRATE)
            // We use a rather large T_slot time because USB-RS485 converters
            // can induce large delays at times.
            .slot_bits(2500)
            // For generating the live-list as fast as possible, set GAP factor to 1.
            .gap_wait_rotations(1)
            .build(),
    );
    // We must not poll() too often or to little. T_slot / 2 seems to be a good compromise.
    let sleep_time: std::time::Duration = (fdl_master.parameters().slot_time() / 2).into();

    println!("Connecting to the bus...");
    let mut phy = phy::LinuxRs485Phy::new(BUS_DEVICE, fdl_master.parameters().baudrate);

    let mut i = 0u64;

    fdl_master.set_online();
    loop {
        fdl_master.poll(profirust::time::Instant::now(), &mut phy, &mut ());

        if i % 100 == 0 {
            let live_list: Vec<_> = fdl_master
                .iter_live_stations()
                .map(|addr| addr.to_string())
                .collect();
            println!("Live Addresses: {}", live_list.join(", "));
        }

        i += 1;
        std::thread::sleep(sleep_time);
    }
}
