use rp_pico as bsp;

use bsp::hal::{self, clocks::init_clocks_and_plls, pac, sio::Sio, watchdog::Watchdog};

use usb_device::{class_prelude::*, prelude::*};
use usbd_serial::SerialPort;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let mut pac = unsafe { pac::Peripherals::steal() };
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    // External high-speed crystal on the pico board is 12Mhz
    let external_xtal_freq_hz = 12_000_000u32;
    let clocks = init_clocks_and_plls(
        external_xtal_freq_hz,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let _pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Set up the USB driver
    let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));

    // Set up the USB Communications Class Device driver
    let mut serial = SerialPort::new(&usb_bus);

    // Create a USB device with a fake VID and PID
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .strings(&[StringDescriptors::default()
            .manufacturer("Rahix Automation")
            .product("Panic Gadget")
            .serial_number("FOOBAR")])
        .unwrap()
        .device_class(2) // from: https://www.usb.org/defined-class-codes
        .build();

    log::error!("Panic!",);
    log::error!("{}", info);

    let start = crate::time::now().unwrap_or(profirust::time::Instant::ZERO);
    loop {
        let now = crate::time::now().unwrap_or(profirust::time::Instant::ZERO);

        // Only print the panic message after two seconds.
        if (now - start).secs() > 2 {
            crate::logger::drain(|buf| match serial.write(buf) {
                Ok(n) => n,
                Err(_) => 0,
            });
        }

        usb_dev.poll(&mut [&mut serial]);

        if (now - start).secs() > 10 {
            hal::rom_data::reset_to_usb_boot(0, 0);
        }
    }
}
