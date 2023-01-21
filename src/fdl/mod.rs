mod master;
mod telegram;

pub use master::{FdlMaster, Parameters};
pub use telegram::{
    Telegram, FunctionCode, RequestType, ResponseState, ResponseStatus, ShortConfirmation,
    DataTelegram, TokenTelegram,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum Baudrate {
    B9600,
    B19200,
    B31250,
    B45450,
    B93750,
    B187500,
    B500000,
    B1500000,
    B3000000,
    B6000000,
    B12000000,
}

impl Baudrate {
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

    pub fn bits_to_time(self, bits: u32) -> crate::time::Duration {
        crate::time::Duration::from_micros(bits as u64 * 1000000 / self.to_rate())
    }
}
