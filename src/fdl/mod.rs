mod master;
mod telegram;

pub use master::FdlMaster;
pub use telegram::{
    AnyTelegram, FunctionCode, RequestType, ResponseState, ResponseStatus, ShortConfirmation,
    Telegram, TokenTelegram,
};

pub enum Baudrate {
    B9600 = 9600,
    B19200 = 19200,
    B31250 = 31250,
    B45450 = 45450,
    B93750 = 93750,
    B187500 = 187500,
    B500000 = 500000,
    B1500000 = 1500000,
    B3000000 = 3000000,
    B6000000 = 6000000,
    B12000000 = 12000000,
}
