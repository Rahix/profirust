# profirust example with rp-pico
This is an example of using profirust on an embedded system - the [Raspberry Pi
Pico] in this case.

### How To Guide
You'll have to set up your Pico for PROFIBUS communication by connecting at
least an RS485 transceiver.  The example expects the following connections:

- UART TX on pin GPIO16 ➡ RS485 transmitter
- UART RX on pin GPIO17 ⬅ RS485 receiver
- GPIO15 ➡  RS485 direction pin

You will most likely also need to add a pull-up resistor to the UART RX line (GPIO17).

And most importantly: Don't forget to add proper bus termination to your PROFIBUS network!

### Flashing & Running
The example is set up to be flashed over USB using the on-board bootloader:

1. Press the BOOT button and connect USB while holding it.
2. `cargo run`

Now open the serial console device (`/dev/ttyACM0` on Linux) to read the logs:

```terminal
❯ picocom -q /dev/ttyACM0
[    2.000036] profirust::fdl::master: FDL master entering state "Online"
[    2.000332] profirust::dp::master: DP master entering state "Operate"
[    2.052125] profirust::fdl::master: Generating new token due to silent bus.
[    2.167298] profirust::dp::peripheral: Peripheral #6 becomes ready for data exchange.
[    3.000048] dp_master_pico: Encoder Counts: 46884
[    4.000025] dp_master_pico: Encoder Counts: 46884
[    5.000023] dp_master_pico: Encoder Counts: 46884
[    6.000035] dp_master_pico: Encoder Counts: 46884
[    7.000030] dp_master_pico: Encoder Counts: 46884
[    8.000026] dp_master_pico: Encoder Counts: 46884
[    9.000022] dp_master_pico: Encoder Counts: 46884
[   10.000027] dp_master_pico: Encoder Counts: 46884
[   11.000030] dp_master_pico: Encoder Counts: 46884
```

[Raspberry Pi PICO]: https://www.raspberrypi.com/products/raspberry-pi-pico/
