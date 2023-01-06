use std::path::Path;

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

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct GenericStationDescription {
    pub gsd_revision: u8,
    pub vendor: String,
    pub model: String,
    pub revision: String,
    pub revision_number: u8,
    pub ident_number: u16,
    pub protocol_ident: ProtocolIdent,
    pub station_type: StationType,
    pub fms_supported: bool,
    pub hardware_release: String,
    pub software_release: String,
    pub redundancy_supported: bool,
    pub repeater_control_signal: RepeaterControlSignal,
    pub pins_24v: Pins24V,
    pub implementation_type: String,
    pub bitmap_device: String,
    pub bitmap_diag: String,
    pub bitmap_sf: String,
    pub freeze_mode_supported: bool,
    pub sync_mode_supported: bool,
    pub auto_baud_supported: bool,
    pub set_slave_addr_supported: bool,
    pub fail_safe: bool,
    pub max_diag_data_length: u8,
    pub max_user_prm_data_length: u8,
    pub module_offset: u8,
    pub slave_family: SlaveFamily,
    pub user_prm_data_length: u8,
    pub default_usr_prm_data: Vec<u8>,
    pub min_slave_intervall_us: u16,
    pub modular_station: bool,
}

pub mod parser {
    #[derive(pest_derive::Parser)]
    #[grammar = "gsd.pest"]
    pub struct GsdParser;
}

pub fn parse_from_file<P: AsRef<Path>>(file: P) -> GenericStationDescription {
    use std::io::Read;
    use pest::Parser;

    let mut f = std::fs::File::open(file.as_ref()).unwrap();
    let mut source = String::new();
    f.read_to_string(&mut source).unwrap();

    let res = match parser::GsdParser::parse(parser::Rule::gsd, &source) {
        Ok(mut res) => res.next().unwrap(),
        Err(e) => panic!("{}", e.with_path(&file.as_ref().to_string_lossy())),
    };

    for f in res.into_inner() {
        dbg!(f);
    }

    todo!();

    Default::default()
}
