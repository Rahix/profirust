// TODO: Remove this once the crate has matured.
#![allow(dead_code)]
#![allow(unused_variables)]

pub mod consts;
pub mod fdl;
pub mod dp;
pub mod phy;
pub mod time;

/// Baudrate for fieldbus communication.
///
/// - PROFIBUS DP networks can run at any of the available baudrates given that all stations
///   support the selected speed.
/// - PROFIBUS PA networks must use `B31250` (31.25 kbit/s).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum Baudrate {
    /// 9.6 kbit/s
    B9600,
    /// 19.2 kbit/s
    B19200,
    /// 31.25 kbit/s
    B31250,
    /// 45.45 kbit/s
    B45450,
    /// 93.75 kbit/s
    B93750,
    /// 187.5 kbit/s
    B187500,
    /// 500 kbit/s
    B500000,
    /// 1.5 Mbit/s
    B1500000,
    /// 3 Mbit/s
    B3000000,
    /// 6 Mbit/s
    B6000000,
    /// 12 Mbit/s
    B12000000,
}

impl Baudrate {
    /// Convert baudrate into its numeric value in bit/s.
    pub fn to_rate(self) -> u64 {
        match self {
            Baudrate::B9600 => 9600,
            Baudrate::B19200 => 19200,
            Baudrate::B31250 => 31250,
            Baudrate::B45450 => 45450,
            Baudrate::B93750 => 93750,
            Baudrate::B187500 => 187500,
            Baudrate::B500000 => 500000,
            Baudrate::B1500000 => 1500000,
            Baudrate::B3000000 => 3000000,
            Baudrate::B6000000 => 6000000,
            Baudrate::B12000000 => 12000000,
        }
    }

    /// At this baudrate, return how long a given number of bits take to transmit.
    pub fn bits_to_time(self, bits: u32) -> crate::time::Duration {
        crate::time::Duration::from_micros(bits as u64 * 1000000 / self.to_rate())
    }

    /// At this baudrate, return how many bits could be transmitted in the given time.
    pub fn time_to_bits(self, time: crate::time::Duration) -> u64 {
        time.total_micros() * self.to_rate() / 1000000
    }
}
