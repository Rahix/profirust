use profirust::phy;
use profirust::phy::ProfibusPhy;

// Bus Parameters
const BUS_DEVICE: &'static str = "/dev/ttyUSB0";
const BAUDRATE: profirust::Baudrate = profirust::Baudrate::B19200;

fn main() {
    env_logger::init();

    println!("PROFIBUS Bus Spy:");

    println!("Connecting to the bus...");
    let mut phy = phy::LinuxRs485Phy::new(BUS_DEVICE, BAUDRATE);

    loop {
        phy.receive_telegram(profirust::time::Instant::now(), |t| println!("{t:?}"));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
