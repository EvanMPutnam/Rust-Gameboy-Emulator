use crate::cartridge::rtc::RtcState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartridgeSave {
    pub ram: Option<Vec<u8>>,
    pub rtc: Option<RtcState>,
}
