//! PROFIBUS Constants

/// Start Delimiter 1
pub const SD1: u8 = 0x10;
/// Start Delimiter 2
pub const SD2: u8 = 0x68;
/// Start Delimiter 3
pub const SD3: u8 = 0xA2;
/// Start Delimiter 4
pub const SD4: u8 = 0xDC;
/// End Delimiter
pub const ED: u8 = 0x16;
/// Short Confirmation
pub const SC: u8 = 0xE5;

/// SAP (Service Access Point) of an FDL master for **Data Exchange**
pub const SAP_MASTER_DATA_EXCHANGE: Option<u8> = None;
/// SAP (Service Access Point) of an FDL master for **DP MS2: Acyclic master class 2**
pub const SAP_MASTER_MS2: Option<u8> = Some(50);
/// SAP (Service Access Point) of an FDL master for **DP MS2: Acyclic master class 1**
pub const SAP_MASTER_MS1: Option<u8> = Some(51);
/// SAP (Service Access Point) of an FDL master for **DP master to master**
pub const SAP_MASTER_MM: Option<u8> = Some(54);
/// SAP (Service Access Point) of an FDL master for **DP MS0: slave handler per DP slave**
pub const SAP_MASTER_MS0: Option<u8> = Some(62);

/// SAP (Service Access Point) of a slave for **Data Exchange**
pub const SAP_SLAVE_DATA_EXCHANGE: Option<u8> = None;
/// SAP (Service Access Point) of a slave for **Get Configuration**
pub const SAP_SLAVE_GET_CFG: Option<u8> = Some(59);
/// SAP (Service Access Point) of a slave for **Slave Diagnosis**
pub const SAP_SLAVE_DIAGNOSIS: Option<u8> = Some(60);
/// SAP (Service Access Point) of a slave for **Set Parameters**
pub const SAP_SLAVE_SET_PRM: Option<u8> = Some(61);
/// SAP (Service Access Point) of a slave for **Check Configuration**
pub const SAP_SLAVE_CHK_CFG: Option<u8> = Some(62);
