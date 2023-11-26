use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

pub mod parser;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum ProtocolIdent {
    #[default]
    ProfibusDp,
    ManufacturerSpecific(u8),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum StationType {
    #[default]
    DpSlave,
    DpMaster,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum RepeaterControlSignal {
    #[default]
    NotConnected,
    Rs485,
    Ttl,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum Pins24V {
    #[default]
    NotConnected,
    Input,
    Output,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
#[repr(u8)]
pub enum MainSlaveFamily {
    #[default]
    General = 0,
    Drives = 1,
    SwitchingDevices = 2,
    IOs = 3,
    Valves = 4,
    Controllers = 5,
    Hmis = 6,
    Encoders = 7,
    NcRc = 8,
    Gateways = 9,
    PLCs = 10,
    IdentSystems = 11,
    PA = 12,
    Reserved(u8),
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct SlaveFamily {
    main: MainSlaveFamily,
    sub: Vec<String>,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SupportedSpeeds: u16 {
        const B9600 = 1 << 1;
        const B19200 = 1 << 2;
        const B31250 = 1 << 3;
        const B45450 = 1 << 4;
        const B93750 = 1 << 5;
        const B187500 = 1 << 6;
        const B500000 = 1 << 7;
        const B1500000 = 1 << 8;
        const B3000000 = 1 << 9;
        const B6000000 = 1 << 10;
        const B12000000 = 1 << 11;
    }
}

impl Default for SupportedSpeeds {
    fn default() -> Self {
        SupportedSpeeds::empty()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MaxTsdr {
    /// Maximum response time (in bits) at 9.6 kbit/s
    pub b9600: u16,
    /// Maximum response time (in bits) at 19.2 kbit/s
    pub b19200: u16,
    /// Maximum response time (in bits) at 31.25 kbit/s
    pub b31250: u16,
    /// Maximum response time (in bits) at 45.45 kbit/s
    pub b45450: u16,
    /// Maximum response time (in bits) at 93.75 kbit/s
    pub b93750: u16,
    /// Maximum response time (in bits) at 187.5 kbit/s
    pub b187500: u16,
    /// Maximum response time (in bits) at 500 kbit/s
    pub b500000: u16,
    /// Maximum response time (in bits) at 1.5 Mbit/s
    pub b1500000: u16,
    /// Maximum response time (in bits) at 3 Mbit/s
    pub b3000000: u16,
    /// Maximum response time (in bits) at 6 Mbit/s
    pub b6000000: u16,
    /// Maximum response time (in bits) at 12 Mbit/s
    pub b12000000: u16,
}

impl Default for MaxTsdr {
    fn default() -> Self {
        Self {
            b9600: 60,
            b19200: 60,
            b31250: 60,
            b45450: 60,
            b93750: 60,
            b187500: 60,
            b500000: 100,
            b1500000: 150,
            b3000000: 250,
            b6000000: 450,
            b12000000: 800,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UserPrmDataType {
    Unsigned8,
    Unsigned16,
    Unsigned32,
    Signed8,
    Signed16,
    Signed32,
    Bit(u8),
    BitArea(u8, u8),
}

impl UserPrmDataType {
    pub fn size(self) -> usize {
        match self {
            UserPrmDataType::Unsigned8 => 1,
            UserPrmDataType::Unsigned16 => 2,
            UserPrmDataType::Unsigned32 => 4,
            UserPrmDataType::Signed8 => 1,
            UserPrmDataType::Signed16 => 2,
            UserPrmDataType::Signed32 => 4,
            UserPrmDataType::Bit(_) => 1,
            UserPrmDataType::BitArea(_, _) => 1,
        }
    }

    pub fn write_value_to_slice(self, value: i64, s: &mut [u8]) {
        match self {
            UserPrmDataType::Unsigned8 => {
                assert!(0 <= value && value <= 255);
                s[..1].copy_from_slice(&(value as u8).to_be_bytes());
            }
            UserPrmDataType::Unsigned16 => {
                assert!(0 <= value && value <= 65535);
                s[..2].copy_from_slice(&(value as u16).to_be_bytes());
            }
            UserPrmDataType::Unsigned32 => {
                assert!(0 <= value && value <= 4294967295);
                s[..4].copy_from_slice(&(value as u32).to_be_bytes());
            }
            UserPrmDataType::Signed8 => {
                assert!(-127 <= value && value <= 127);
                s[..1].copy_from_slice(&(value as i8).to_be_bytes());
            }
            UserPrmDataType::Signed16 => {
                assert!(-32767 <= value && value <= 32767);
                s[..2].copy_from_slice(&(value as i16).to_be_bytes());
            }
            UserPrmDataType::Signed32 => {
                assert!(2147483647 <= value && value <= 2147483647);
                s[..4].copy_from_slice(&(value as i32).to_be_bytes());
            }
            UserPrmDataType::Bit(b) => {
                assert!(value == 0 || value == 1);
                s[0] |= (value as u8) << b;
            }
            UserPrmDataType::BitArea(first, last) => {
                let bit_size = last - first + 1;
                assert!(value >= 0 && value < 2i64.pow(bit_size as u32));
                s[0] = (value as u8) << first;
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PrmValueConstraint {
    MinMax(i64, i64),
    Enum(Vec<i64>),
}

impl PrmValueConstraint {
    pub fn is_valid(&self, value: i64) -> bool {
        match self {
            PrmValueConstraint::MinMax(min, max) => *min <= value && value <= *max,
            PrmValueConstraint::Enum(values) => values.contains(&value),
        }
    }

    pub fn assert_valid(&self, value: i64) {
        match self {
            PrmValueConstraint::MinMax(min, max) => {
                assert!(
                    *min <= value && value <= *max,
                    "value {value} not in range {min}..={max}",
                );
            }
            PrmValueConstraint::Enum(values) => {
                assert!(
                    values.contains(&value),
                    "value {value} not in set {values:?}",
                );
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct UserPrmDataDefinition {
    pub name: String,
    pub data_type: UserPrmDataType,
    pub default_value: i64,
    pub constraint: PrmValueConstraint,
    pub text_ref: Option<Arc<BTreeMap<String, i64>>>,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct UserPrmData {
    pub length: u8,
    pub data_const: Vec<(usize, Vec<u8>)>,
    pub data_ref: Vec<(usize, Arc<UserPrmDataDefinition>)>,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Module {
    pub name: String,
    pub config: Vec<u8>,
    pub reference: Option<u32>,
    pub module_prm_data: UserPrmData,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct UnitDiagBitInfo {
    pub text: String,
    pub help: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct UnitDiagArea {
    pub first: u16,
    pub last: u16,
    pub values: BTreeMap<u16, String>,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct UnitDiag {
    pub bits: BTreeMap<u32, UnitDiagBitInfo>,
    pub not_bits: BTreeMap<u32, UnitDiagBitInfo>,
    pub areas: Vec<UnitDiagArea>,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct GenericStationDescription {
    pub gsd_revision: u8,
    pub vendor: String,
    pub model: String,
    pub revision: String,
    pub revision_number: u8,
    pub ident_number: u16,
    // pub protocol_ident: ProtocolIdent,
    // pub station_type: StationType,
    // pub fms_supported: bool,
    pub hardware_release: String,
    pub software_release: String,
    // pub redundancy_supported: bool,
    // pub repeater_control_signal: RepeaterControlSignal,
    // pub pins_24v: Pins24V,
    pub implementation_type: String,
    // pub bitmap_device: String,
    // pub bitmap_diag: String,
    // pub bitmap_sf: String,
    // pub freeze_mode_supported: bool,
    // pub sync_mode_supported: bool,
    // pub auto_baud_supported: bool,
    // pub set_slave_addr_supported: bool,
    pub fail_safe: bool,
    // pub max_diag_data_length: u8,
    // pub max_user_prm_data_length: u8,
    // pub module_offset: u8,
    // pub slave_family: SlaveFamily,
    // pub user_prm_data_length: u8,
    // pub default_usr_prm_data: Vec<u8>,
    // pub min_slave_intervall_us: u16,
    pub modular_station: bool,
    pub max_modules: u8,
    pub max_input_length: u8,
    pub max_output_length: u8,
    pub max_data_length: u8,
    pub supported_speeds: SupportedSpeeds,
    pub max_tsdr: MaxTsdr,
    //
    pub available_modules: Vec<Module>,
    pub user_prm_data: UserPrmData,
    //
    pub unit_diag: UnitDiag,
}

pub struct PrmBuilder<'a> {
    desc: &'a UserPrmData,
    prm: Vec<u8>,
}

impl<'a> PrmBuilder<'a> {
    pub fn new(desc: &'a UserPrmData) -> Self {
        let mut this = Self {
            desc,
            prm: Vec::new(),
        };
        this.write_const_prm_data();
        this.write_default_prm_data();
        this
    }

    fn update_prm_data_len(&mut self, offset: usize, size: usize) {
        if self.prm.len() < (offset + size) {
            for _ in 0..((offset + size) - self.prm.len()) {
                self.prm.push(0x00);
            }
        }
    }

    fn write_const_prm_data(&mut self) {
        for (offset, data_const) in self.desc.data_const.iter() {
            self.update_prm_data_len(*offset, data_const.len());
            self.prm[*offset..(offset + data_const.len())].copy_from_slice(data_const);
        }
    }

    fn write_default_prm_data(&mut self) {
        for (offset, data_ref) in self.desc.data_ref.iter() {
            let size = data_ref.data_type.size();
            self.update_prm_data_len(*offset, size);
            data_ref
                .data_type
                .write_value_to_slice(data_ref.default_value, &mut self.prm[(*offset as usize)..]);
        }
    }

    pub fn set_prm(&mut self, prm: &str, value: i64) -> &mut Self {
        let (offset, data_ref) = self
            .desc
            .data_ref
            .iter()
            .find(|(_, r)| r.name == prm)
            .unwrap();
        data_ref.constraint.assert_valid(value);
        data_ref
            .data_type
            .write_value_to_slice(value, &mut self.prm[(*offset as usize)..]);
        self
    }

    pub fn set_prm_from_text(&mut self, prm: &str, value: &str) -> &mut Self {
        let (offset, data_ref) = self
            .desc
            .data_ref
            .iter()
            .find(|(_, r)| r.name == prm)
            .unwrap();
        let text_ref = data_ref.text_ref.as_ref().unwrap();
        let value = *text_ref.get(value).unwrap();
        data_ref.constraint.assert_valid(value);
        data_ref
            .data_type
            .write_value_to_slice(value, &mut self.prm[(*offset as usize)..]);
        self
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.prm
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.prm
    }
}

pub fn parse_from_file<P: AsRef<Path>>(file: P) -> GenericStationDescription {
    use std::io::Read;

    let mut f = std::fs::File::open(file.as_ref()).unwrap();
    let mut source_bytes = Vec::new();
    f.read_to_end(&mut source_bytes).unwrap();
    let source = String::from_utf8_lossy(&source_bytes);

    parser::parse(file.as_ref(), &source)
}
