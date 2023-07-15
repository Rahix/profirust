use std::path::Path;

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

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Module {
    name: String,
    config: Vec<u8>,
    input_length: u8,
    output_length: u8,
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
    // pub fail_safe: bool,
    // pub max_diag_data_length: u8,
    // pub max_user_prm_data_length: u8,
    // pub module_offset: u8,
    // pub slave_family: SlaveFamily,
    // pub user_prm_data_length: u8,
    // pub default_usr_prm_data: Vec<u8>,
    // pub min_slave_intervall_us: u16,
    pub modular_station: bool,
    // pub max_modules: u8,
    // pub max_input_length: u8,
    // pub max_output_length: u8,
    // pub max_data_length: u8,
    pub supported_speeds: SupportedSpeeds,
    pub max_tsdr: MaxTsdr,
    //
    pub available_modules: Vec<Module>,
}

pub fn parse_from_file<P: AsRef<Path>>(file: P) -> GenericStationDescription {
    use std::io::Read;

    let mut f = std::fs::File::open(file.as_ref()).unwrap();
    let mut source_bytes = Vec::new();
    f.read_to_end(&mut source_bytes).unwrap();
    let source = String::from_utf8_lossy(&source_bytes);

    parser::parse(file.as_ref(), &source)
}
