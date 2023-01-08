#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum RequestType {
    /// Clock Value
    ClockValue = 0 | 1 << 7,
    /// Time Event
    TimeEvent = 0,
    /// SDA (Send Data Acknowledged) with low priority
    SdaLow = 3,
    /// SDN (Send Data Not acknowledged) with low priority
    SdnLow = 4,
    /// SDA (Send Data Acknowledged) with high priority
    SdaHigh = 5,
    /// SDN (Send Data Not acknowledged) with high priority
    SdnHigh = 6,
    /// SRD (Send Request Data) with multicast reply
    MulticastSrd = 7,
    /// Request FDL status
    FdlStatus = 9,
    // SRD (Send Request Data) with low priority
    SrdLow = 12,
    // SRD (Send Request Data) with high priority
    SrdHigh = 13,
    /// Request ident
    Ident = 14,
    /// Request LSAP status (deprecated)
    LsapStatus = 15,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum ResponseState {
    /// Slave
    Slave = 0,
    /// Master is not ready
    MasterNotReady = 1,
    /// Master is ready but has no token
    MasterWithoutToken = 2,
    /// Master is ready and in token ring
    MasterInRing = 3,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum ResponseStatus {
    /// OK
    Ok = 0,
    /// UE = User error
    UserError = 1,
    /// RR = No resources
    NoResources = 2,
    /// RS = SAP not enabled
    SapNotEnabled = 3,
    /// DL = Data Low
    DataLow = 8,
    /// NR = No response data ready
    NoDataReady = 9,
    /// DH = Data High
    DataHigh = 10,
    /// RDL = Data not received and data low
    NotReceivedDataLow = 12,
    /// RDH = Data not received and data high
    NotReceivedDataHigh = 13,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FunctionCode {
    /// This marks a request telegram
    Request {
        fcv: bool,
        fcb: bool,
        req: RequestType,
    },
    /// This marks a response telegram
    Response {
        state: ResponseState,
        status: ResponseStatus,
    },
}

impl FunctionCode {
    pub fn to_byte(self) -> u8 {
        match self {
            FunctionCode::Request { fcv, fcb, req } => {
                (1 << 6) | req as u8 | ((fcv as u8) << 4) | ((fcb as u8) << 5)
            }
            FunctionCode::Response { state, status } => ((state as u8) << 4) | status as u8,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Telegram<'a> {
    /// Destination Address
    pub da: u8,
    /// Source Address
    pub sa: u8,
    /// Destination "Service Access Point"
    pub dsap: Option<u8>,
    /// Source "Service Access Point"
    pub ssap: Option<u8>,
    /// Function Code
    pub fc: FunctionCode,
    /// Protocol Data Unit - Payload of the telegram
    pub pdu: &'a [u8],
}

impl Telegram<'_> {
    /// Generate an FDL Status request telegram
    pub fn fdl_status(da: u8, sa: u8) -> Self {
        Self {
            da,
            sa,
            dsap: None,
            ssap: None,
            fc: FunctionCode::Request {
                fcv: false,
                fcb: false,
                req: RequestType::FdlStatus,
            },
            pdu: &[],
        }
    }

    pub fn serialize(&self, buffer: &mut [u8]) -> usize {
        let length_byte =
            self.pdu.len() + self.dsap.is_some() as usize + self.ssap.is_some() as usize + 3;

        let mut cursor = 0;

        let sc = match length_byte {
            // no PDU
            3 => crate::consts::SD1,
            // exactly 8 bytes content (3 + 8)
            11 => crate::consts::SD3,
            // all other lengths
            _ => crate::consts::SD2,
        };
        buffer[cursor] = sc;
        cursor += 1;
        if sc == crate::consts::SD2 {
            assert!(length_byte <= 249);
            buffer[cursor] = length_byte as u8;
            buffer[cursor + 1] = length_byte as u8;
            buffer[cursor + 2] = sc;
            cursor += 3;
        }

        let checksum_start = cursor;

        buffer[cursor] = self.da;
        buffer[cursor + 1] = self.sa;
        buffer[cursor + 2] = self.fc.to_byte();
        cursor += 3;

        buffer[cursor..cursor + self.pdu.len()].copy_from_slice(self.pdu);
        cursor += self.pdu.len();

        buffer[cursor] = buffer[checksum_start..cursor]
            .iter()
            .copied()
            .fold(0, u8::wrapping_add);
        buffer[cursor + 1] = crate::consts::ED;
        cursor += 2;

        cursor
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TokenTelegram {
    /// Destination Address
    pub da: u8,
    /// Source Address
    pub sa: u8,
}

impl TokenTelegram {
    pub fn new(da: u8, sa: u8) -> Self {
        Self { da, sa }
    }

    pub fn serialize(&self, buffer: &mut [u8]) -> usize {
        buffer[0] = crate::consts::SD4;
        buffer[1] = self.da;
        buffer[2] = self.sa;
        3
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ShortConfirmation;

impl ShortConfirmation {
    pub fn serialize(&self, buffer: &mut [u8]) -> usize {
        buffer[0] = crate::consts::SC;
        1
    }
}

pub enum AnyTelegram<'a> {
    Telegram(Telegram<'a>),
    TokenTelegram(TokenTelegram),
    ShortConfirmation(ShortConfirmation),
}

impl<'a> From<Telegram<'a>> for AnyTelegram<'a> {
    fn from(value: Telegram<'a>) -> Self {
        Self::Telegram(value)
    }
}

impl From<TokenTelegram> for AnyTelegram<'_> {
    fn from(value: TokenTelegram) -> Self {
        Self::TokenTelegram(value)
    }
}

impl From<ShortConfirmation> for AnyTelegram<'_> {
    fn from(value: ShortConfirmation) -> Self {
        Self::ShortConfirmation(value)
    }
}

impl AnyTelegram<'_> {
    pub fn serialize(&self, buffer: &mut [u8]) -> usize {
        match self {
            Self::Telegram(t) => t.serialize(buffer),
            Self::TokenTelegram(t) => t.serialize(buffer),
            Self::ShortConfirmation(t) => t.serialize(buffer),
        }
    }

    pub fn deserialize(buffer: &[u8]) -> Option<Result<Self, ()>> {
        if buffer.len() == 0 {
            return None;
        }

        match buffer[0] {
            crate::consts::SC => Some(Ok(ShortConfirmation.into())),
            _ => Some(Err(())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fdl_status() {
        let mut buffer = vec![0x00; 256];
        let length = Telegram::fdl_status(34, 2).serialize(&mut buffer);
        let msg = &buffer[..length];
        let expected = &[0x10, 0x22, 0x02, 0x49, 0x6D, 0x16];
        assert_eq!(msg, expected);
    }
}
