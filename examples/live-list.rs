use profirust::fdl;
use profirust::phy;

// Bus Parameters
const MASTER_ADDRESS: u8 = 3;
const BUS_DEVICE: &'static str = "/dev/ttyUSB0";
const BAUDRATE: profirust::Baudrate = profirust::Baudrate::B500000;

fn main() -> ! {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_micros()
        .init();

    log::info!("PROFIBUS Live List:");

    let mut live_list = fdl::live_list::LiveList::new();

    let mut fdl = fdl::FdlActiveStation::new(
        fdl::ParametersBuilder::new(MASTER_ADDRESS, BAUDRATE)
            // We use a rather large T_slot time because USB-RS485 converters
            // can induce large delays at times.
            .slot_bits(4000)
            .max_retry_limit(3)
            // For generating the live-list as fast as possible, set GAP factor to 1.
            .gap_wait_rotations(1)
            .build(),
    );
    // Read more about timing considerations in the SerialPortPhy documentation.
    let sleep_time = std::time::Duration::from_micros(3500);

    log::warn!(
        "This station has address #{}.  No other station with this address shall be present.",
        fdl.parameters().address
    );

    log::info!("Connecting to the bus...");
    let mut phy = phy::SerialPortPhy::new(BUS_DEVICE, fdl.parameters().baudrate);

    let mut last = profirust::time::Instant::now();

    fdl.set_online();
    loop {
        fdl.poll(profirust::time::Instant::now(), &mut phy, &mut live_list);

        let event = live_list.take_last_event();

        match event {
            Some(fdl::live_list::StationEvent::Discovered(station)) => {
                log::info!("Discovered #{} ({:?})", station.address, station.state);
            }
            Some(fdl::live_list::StationEvent::Lost(station_address)) => {
                log::info!("Lost station #{station_address}");
            }
            None => (),
        }

        if (profirust::time::Instant::now() - last).secs() > 5 {
            let live_list: Vec<_> = live_list
                .iter_stations()
                .map(|addr| format!("#{}", addr))
                .collect();
            log::info!("Live Stations: {}", live_list.join(", "));
            last = profirust::time::Instant::now();
        }

        std::thread::sleep(sleep_time);
    }
}
