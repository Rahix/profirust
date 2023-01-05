use std::path::Path;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GenericStationDescription {
}

impl GenericStationDescription {
    pub fn parse_from_file<P: AsRef<Path>>(file: P) -> Self {
        Self {}
    }
}
