use profirust::fdl;
use profirust::phy;

// Bus Parameters
const MASTER_ADDRESS: u8 = 3;
const BUS_DEVICE: &'static str = "/dev/ttyUSB0";
const BAUDRATE: profirust::Baudrate = profirust::Baudrate::B500000;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("trace"))
        .format_timestamp_micros()
        .init();

    println!("PROFIBUS Live List:");

    log::warn!("FdlActiveStation doesn't have a live-list feature at this time!");
    log::warn!("You can see what stations are active from the bus trace below...");

    let mut fdl = fdl::FdlActiveStation::new(
        fdl::ParametersBuilder::new(MASTER_ADDRESS, BAUDRATE)
            // We use a rather large T_slot time because USB-RS485 converters
            // can induce large delays at times.
            .slot_bits(2500)
            // For generating the live-list as fast as possible, set GAP factor to 1.
            .gap_wait_rotations(1)
            .build(),
    );
    // We must not poll() too often or to little. T_slot / 2 seems to be a good compromise.
    let sleep_time: std::time::Duration = (fdl.parameters().slot_time() / 2).into();

    println!("Connecting to the bus...");
    let mut phy = phy::LinuxRs485Phy::new(BUS_DEVICE, fdl.parameters().baudrate);

    let mut i = 0u64;

    fdl.set_online();
    loop {
        fdl.poll(profirust::time::Instant::now(), &mut phy, &mut ());

        // TODO: Update once new live-list is available.
        // if i % 100 == 0 {
        //     let live_list: Vec<_> = fdl
        //         .iter_live_stations()
        //         .map(|addr| addr.to_string())
        //         .collect();
        //     println!("Live Addresses: {}", live_list.join(", "));
        // }

        i += 1;
        std::thread::sleep(sleep_time);
    }
}
