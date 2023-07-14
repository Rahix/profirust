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

impl RequestType {
    pub fn from_u8(b: u8) -> Option<RequestType> {
        match b {
            0x80 => Some(Self::ClockValue),
            0 => Some(Self::TimeEvent),
            3 => Some(Self::SdaLow),
            4 => Some(Self::SdnLow),
            5 => Some(Self::SdaHigh),
            6 => Some(Self::SdnHigh),
            7 => Some(Self::MulticastSrd),
            9 => Some(Self::FdlStatus),
            12 => Some(Self::SrdLow),
            13 => Some(Self::SrdHigh),
            14 => Some(Self::Ident),
            15 => Some(Self::LsapStatus),
            _ => None,
        }
    }
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

impl ResponseState {
    pub fn from_u8(b: u8) -> Option<ResponseState> {
        match b {
            0 => Some(Self::Slave),
            1 => Some(Self::MasterNotReady),
            2 => Some(Self::MasterWithoutToken),
            3 => Some(Self::MasterInRing),
            _ => None,
        }
    }
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

impl ResponseStatus {
    pub fn from_u8(b: u8) -> Option<ResponseStatus> {
        match b {
            0 => Some(Self::Ok),
            1 => Some(Self::UserError),
            2 => Some(Self::NoResources),
            3 => Some(Self::SapNotEnabled),
            8 => Some(Self::DataLow),
            9 => Some(Self::NoDataReady),
            10 => Some(Self::DataHigh),
            12 => Some(Self::NotReceivedDataLow),
            13 => Some(Self::NotReceivedDataHigh),
            _ => None,
        }
    }
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

    pub fn from_byte(b: u8) -> Result<Self, ()> {
        if b & (1 << 6) != 0 {
            let fcv = b & (1 << 4) != 0;
            let fcb = b & (1 << 5) != 0;
            let req = RequestType::from_u8(b & 0x8F).ok_or(())?;
            Ok(Self::Request { fcv, fcb, req })
        } else {
            let state = ResponseState::from_u8((b & 0x30) >> 4).ok_or(())?;
            let status = ResponseStatus::from_u8(b & 0x0F).ok_or(())?;
            Ok(Self::Response { state, status })
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DataTelegram<'a> {
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

impl DataTelegram<'_> {
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

        let da_ext = if self.dsap.is_some() { 0x80 } else { 0x00 };
        buffer[cursor] = self.da | da_ext;
        let sa_ext = if self.ssap.is_some() { 0x80 } else { 0x00 };
        buffer[cursor + 1] = self.sa | sa_ext;
        buffer[cursor + 2] = self.fc.to_byte();
        cursor += 3;

        if let Some(dsap) = self.dsap {
            buffer[cursor] = dsap;
            cursor += 1;
        }
        if let Some(ssap) = self.ssap {
            buffer[cursor] = ssap;
            cursor += 1;
        }

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

    pub fn deserialize(mut buffer: &[u8]) -> Option<Result<Self, ()>> {
        if buffer.len() < 6 {
            return None;
        }

        let length = match buffer[0] {
            crate::consts::SD1 => 0,
            crate::consts::SD2 => {
                let l1 = buffer[1];
                let l2 = buffer[2];
                buffer = &buffer[3..];
                if l1 != l2 {
                    return Some(Err(()));
                }
                l1 - 3
            }
            crate::consts::SD3 => 8,
            _ => unreachable!(),
        };
        let mut length = length as usize;

        if buffer.len() < length as usize + 2 {
            return None;
        }

        let buffer_checksum = &buffer[1..];
        let checksum_length = length + 3;

        let da = buffer[1];
        let (has_dsap, da) = if da & 0x80 != 0 {
            (true, da & !0x80)
        } else {
            (false, da)
        };
        let sa = buffer[2];
        let (has_ssap, sa) = if sa & 0x80 != 0 {
            (true, sa & !0x80)
        } else {
            (false, sa)
        };

        let fc = match FunctionCode::from_byte(buffer[3]) {
            Ok(fc) => fc,
            Err(_) => {
                log::debug!("Unparseable function code");
                return Some(Err(()));
            }
        };

        let mut buffer = &buffer[4..];

        let dsap = if has_dsap {
            let dsap = buffer[0];
            length -= 1;
            buffer = &buffer[1..];
            Some(dsap)
        } else {
            None
        };
        let ssap = if has_ssap {
            let ssap = buffer[0];
            length -= 1;
            buffer = &buffer[1..];
            Some(ssap)
        } else {
            None
        };

        let pdu = &buffer[..length];

        let checksum_received = buffer[length];
        let checksum_calculated = buffer_checksum[..checksum_length]
            .iter()
            .copied()
            .fold(0, u8::wrapping_add);

        if checksum_received != checksum_calculated {
            log::debug!("Checksum mismatch");
            return Some(Err(()));
        }

        if buffer[length + 1] != crate::consts::ED {
            log::debug!("No end delimiter");
            return Some(Err(()));
        }

        Some(Ok(DataTelegram {
            da,
            sa,
            dsap,
            ssap,
            fc,
            pdu: &[],
        }))
    }
}

impl DataTelegram<'_> {
    /// Generate an FDL Status request telegram
    pub fn new_fdl_status_request(da: u8, sa: u8) -> Self {
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

    /// Returns the source address if this telegram is an FDL status request for us.
    pub fn is_fdl_status_request(&self) -> Option<u8> {
        if matches!(
            self.fc,
            FunctionCode::Request {
                req: RequestType::FdlStatus,
                ..
            }
        ) {
            Some(self.sa)
        } else {
            None
        }
    }

    /// Generate an FDL Status response telegram
    pub fn new_fdl_status_response(
        da: u8,
        sa: u8,
        state: ResponseState,
        status: ResponseStatus,
    ) -> Self {
        Self {
            da,
            sa,
            dsap: None,
            ssap: None,
            fc: FunctionCode::Response { state, status },
            pdu: &[],
        }
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

    pub fn deserialize(buffer: &[u8]) -> Option<Result<Self, ()>> {
        if buffer.len() < 3 {
            return None;
        }

        // already checked by calling code
        debug_assert!(buffer[0] == crate::consts::SD4);
        let da = buffer[1];
        let sa = buffer[2];

        Some(Ok(Self { da, sa }))
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Telegram<'a> {
    Data(DataTelegram<'a>),
    Token(TokenTelegram),
    ShortConfirmation(ShortConfirmation),
}

impl<'a> From<DataTelegram<'a>> for Telegram<'a> {
    fn from(value: DataTelegram<'a>) -> Self {
        Self::Data(value)
    }
}

impl From<TokenTelegram> for Telegram<'_> {
    fn from(value: TokenTelegram) -> Self {
        Self::Token(value)
    }
}

impl From<ShortConfirmation> for Telegram<'_> {
    fn from(value: ShortConfirmation) -> Self {
        Self::ShortConfirmation(value)
    }
}

impl Telegram<'_> {
    pub fn serialize(&self, buffer: &mut [u8]) -> usize {
        match self {
            Self::Data(t) => t.serialize(buffer),
            Self::Token(t) => t.serialize(buffer),
            Self::ShortConfirmation(t) => t.serialize(buffer),
        }
    }

    pub fn deserialize(buffer: &[u8]) -> Option<Result<Self, ()>> {
        if buffer.len() == 0 {
            return None;
        }

        match buffer[0] {
            crate::consts::SC => Some(Ok(ShortConfirmation.into())),
            crate::consts::SD4 => TokenTelegram::deserialize(buffer).map(|v| v.map(|v| v.into())),
            crate::consts::SD1 | crate::consts::SD2 | crate::consts::SD3 => {
                DataTelegram::deserialize(buffer).map(|v| v.map(|v| v.into()))
            }
            _ => Some(Err(())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_fdl_status_telegram() {
        let mut buffer = vec![0x00; 256];
        let length = DataTelegram::new_fdl_status_request(34, 2).serialize(&mut buffer);
        let msg = &buffer[..length];
        let expected = &[0x10, 0x22, 0x02, 0x49, 0x6D, 0x16];
        assert_eq!(msg, expected);
    }

    #[test]
    fn parse_fdl_status_telegram() {
        let _ = env_logger::try_init();
        let msg = &[0x10, 0x22, 0x02, 0x49, 0x6D, 0x16];
        let telegram = Telegram::deserialize(msg).unwrap().unwrap();
        assert_eq!(telegram, DataTelegram::new_fdl_status_request(34, 2).into());
    }

    #[test]
    fn parse_fdl_response_telegram() {
        let _ = env_logger::try_init();
        let msg = &[0x10, 0x02, 0x22, 0x00, 0x24, 0x16];
        let telegram = Telegram::deserialize(msg).unwrap().unwrap();
        dbg!(telegram);
    }

    mod enum_consistency {
        use super::*;

        #[test]
        fn request_type() {
            for req_type in [
                RequestType::ClockValue,
                RequestType::TimeEvent,
                RequestType::SdaLow,
                RequestType::SdnLow,
                RequestType::SdaHigh,
                RequestType::SdnHigh,
                RequestType::MulticastSrd,
                RequestType::FdlStatus,
                RequestType::SrdLow,
                RequestType::SrdHigh,
                RequestType::Ident,
                RequestType::LsapStatus,
            ]
            .into_iter()
            {
                let int_value = req_type as u8;
                let req_type_again = RequestType::from_u8(int_value);
                assert_eq!(Some(req_type), req_type_again);
            }
        }

        #[test]
        fn response_state() {
            for resp_state in [
                ResponseState::Slave,
                ResponseState::MasterNotReady,
                ResponseState::MasterWithoutToken,
                ResponseState::MasterInRing,
            ].into_iter() {
                let int_value = resp_state as u8;
                let resp_state_again = ResponseState::from_u8(int_value);
                assert_eq!(Some(resp_state), resp_state_again);
            }
        }
    }
}
