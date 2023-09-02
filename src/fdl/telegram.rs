#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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

/// Frame Count Bit
///
/// The FCB (Frame Count Bit) is used to detect lost messages and prevent duplication on either
/// side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
#[repr(u8)]
pub enum FrameCountBit {
    #[default]
    First,
    High,
    Low,
    Inactive,
}

impl FrameCountBit {
    pub fn reset(&mut self) {
        *self = FrameCountBit::First;
    }

    pub fn cycle(&mut self) {
        *self = match self {
            FrameCountBit::First => FrameCountBit::Low,
            FrameCountBit::High => FrameCountBit::Low,
            FrameCountBit::Low => FrameCountBit::High,
            FrameCountBit::Inactive => panic!("FCB must not be inactive to be cycled!"),
        }
    }

    pub fn fcb(self) -> bool {
        match self {
            FrameCountBit::First => true,
            FrameCountBit::High => true,
            FrameCountBit::Low => false,
            FrameCountBit::Inactive => false,
        }
    }

    pub fn fcv(self) -> bool {
        match self {
            FrameCountBit::First => false,
            FrameCountBit::High => true,
            FrameCountBit::Low => true,
            FrameCountBit::Inactive => false,
        }
    }

    pub fn from_fcv_fcb(fcv: bool, fcb: bool) -> FrameCountBit {
        match (fcv, fcb) {
            (false, false) => FrameCountBit::Inactive,
            (false, true) => FrameCountBit::First,
            (true, true) => FrameCountBit::High,
            (true, false) => FrameCountBit::Low,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum FunctionCode {
    /// This marks a request telegram
    Request {
        fcb: FrameCountBit,
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
            FunctionCode::Request { fcb, req } => {
                (1 << 6) | req as u8 | ((fcb.fcv() as u8) << 4) | ((fcb.fcb() as u8) << 5)
            }
            FunctionCode::Response { state, status } => ((state as u8) << 4) | status as u8,
        }
    }

    pub fn from_byte(b: u8) -> Result<Self, ()> {
        if b & (1 << 6) != 0 {
            let fcv = b & (1 << 4) != 0;
            let fcb = b & (1 << 5) != 0;
            let req = RequestType::from_u8(b & 0x8F).ok_or(())?;
            Ok(Self::Request {
                fcb: FrameCountBit::from_fcv_fcb(fcv, fcb),
                req,
            })
        } else {
            let state = ResponseState::from_u8((b & 0x30) >> 4).ok_or(())?;
            let status = ResponseStatus::from_u8(b & 0x0F).ok_or(())?;
            Ok(Self::Response { state, status })
        }
    }

    pub fn new_srd_low(fcb: FrameCountBit) -> Self {
        Self::Request {
            fcb,
            req: RequestType::SrdLow,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DataTelegramHeader {
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
}

impl DataTelegramHeader {
    pub fn serialize<F>(&self, buffer: &mut [u8], pdu_len: usize, write_pdu: F) -> usize
    where
        F: FnOnce(&mut [u8]),
    {
        let length_byte =
            pdu_len + usize::from(self.dsap.is_some()) + usize::from(self.ssap.is_some()) + 3;

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
            buffer[cursor] = u8::try_from(length_byte).unwrap();
            buffer[cursor + 1] = u8::try_from(length_byte).unwrap();
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

        let pdu_buffer = &mut buffer[cursor..cursor + pdu_len];
        pdu_buffer.fill(0x00);
        write_pdu(pdu_buffer);
        cursor += pdu_len;

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
pub struct DataTelegram<'a> {
    /// Telegram Header Information
    pub h: DataTelegramHeader,
    /// Protocol Data Unit - Payload of the telegram
    pub pdu: &'a [u8],
}

impl<'a> DataTelegram<'a> {
    pub fn deserialize(mut buffer: &'a [u8]) -> Option<Result<Self, ()>> {
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
        let mut length = usize::from(length);

        if buffer.len() < length + 2 {
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
            h: DataTelegramHeader {
                da,
                sa,
                dsap,
                ssap,
                fc,
            },
            pdu,
        }))
    }
}

impl DataTelegram<'_> {
    /// Returns the source address if this telegram is an FDL status request for us.
    pub fn is_fdl_status_request(&self) -> Option<u8> {
        if matches!(
            self.h.fc,
            FunctionCode::Request {
                req: RequestType::FdlStatus,
                ..
            }
        ) {
            Some(self.h.sa)
        } else {
            None
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

#[derive(PartialEq, Eq, Clone)]
pub enum Telegram<'a> {
    Data(DataTelegram<'a>),
    Token(TokenTelegram),
    ShortConfirmation(ShortConfirmation),
}

impl core::fmt::Debug for Telegram<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Telegram::Data(d) => core::fmt::Debug::fmt(d, f),
            Telegram::Token(t) => core::fmt::Debug::fmt(t, f),
            Telegram::ShortConfirmation(s) => core::fmt::Debug::fmt(s, f),
        }
    }
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

impl<'a> Telegram<'a> {
    pub fn deserialize(buffer: &'a [u8]) -> Option<Result<Self, ()>> {
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

pub struct TelegramTx<'a> {
    buf: &'a mut [u8],
}

pub struct TelegramTxResponse {
    bytes_sent: usize,
}

impl<'a> TelegramTx<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf }
    }

    pub fn send_token_telegram(self, da: u8, sa: u8) -> TelegramTxResponse {
        let token_telegram = TokenTelegram::new(da, sa);
        TelegramTxResponse::new(token_telegram.serialize(self.buf))
    }

    pub fn send_short_confirmation(self) -> TelegramTxResponse {
        let sc_telegram = ShortConfirmation;
        TelegramTxResponse::new(sc_telegram.serialize(self.buf))
    }

    pub fn send_data_telegram<F: FnOnce(&mut [u8])>(
        self,
        header: DataTelegramHeader,
        pdu_len: usize,
        write_pdu: F,
    ) -> TelegramTxResponse {
        TelegramTxResponse::new(header.serialize(self.buf, pdu_len, write_pdu))
    }

    pub fn send_fdl_status_request(self, da: u8, sa: u8) -> TelegramTxResponse {
        self.send_data_telegram(
            DataTelegramHeader {
                da,
                sa,
                dsap: None,
                ssap: None,
                fc: FunctionCode::Request {
                    fcb: FrameCountBit::Inactive,
                    req: RequestType::FdlStatus,
                },
            },
            0,
            |_| (),
        )
    }

    pub fn send_fdl_status_response(
        self,
        da: u8,
        sa: u8,
        state: ResponseState,
        status: ResponseStatus,
    ) -> TelegramTxResponse {
        self.send_data_telegram(
            DataTelegramHeader {
                da,
                sa,
                dsap: None,
                ssap: None,
                fc: FunctionCode::Response { state, status },
            },
            0,
            |_| (),
        )
    }
}

impl TelegramTxResponse {
    pub fn new(bytes_sent: usize) -> Self {
        Self { bytes_sent }
    }
    pub fn bytes_sent(self) -> usize {
        self.bytes_sent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn generate_fdl_status_telegram() {
        let mut buffer = vec![0x00; 256];
        let tx = TelegramTx::new(&mut buffer);
        let length = tx.send_fdl_status_request(34, 2).bytes_sent();
        let msg = &buffer[..length];
        let expected = &[0x10, 0x22, 0x02, 0x49, 0x6D, 0x16];
        assert_eq!(msg, expected);
    }

    #[test]
    fn parse_fdl_status_telegram() {
        let _ = env_logger::try_init();
        let msg = &[0x10, 0x22, 0x02, 0x49, 0x6D, 0x16];
        let telegram = Telegram::deserialize(msg).unwrap().unwrap();
        assert_eq!(
            telegram,
            Telegram::Data(DataTelegram {
                h: DataTelegramHeader {
                    da: 34,
                    sa: 2,
                    dsap: None,
                    ssap: None,
                    fc: FunctionCode::Request {
                        fcb: FrameCountBit::Inactive,
                        req: RequestType::FdlStatus
                    }
                },
                pdu: &[],
            })
        );
    }

    #[test]
    fn parse_fdl_response_telegram() {
        let _ = env_logger::try_init();
        let msg = &[0x10, 0x02, 0x22, 0x00, 0x24, 0x16];
        let telegram = Telegram::deserialize(msg).unwrap().unwrap();
        assert_eq!(
            telegram,
            Telegram::Data(DataTelegram {
                h: DataTelegramHeader {
                    da: 2,
                    sa: 34,
                    dsap: None,
                    ssap: None,
                    fc: FunctionCode::Response {
                        state: ResponseState::Slave,
                        status: ResponseStatus::Ok
                    }
                },
                pdu: &[],
            })
        )
    }

    proptest! {
        #[test]
        fn function_code_serdes(fc in any::<FunctionCode>()) {
            let fc_byte = fc.to_byte();
            let fc_again = FunctionCode::from_byte(fc_byte);
            assert_eq!(Ok(fc), fc_again);
        }
    }
}
