//! Network remotes, independent of DDC/CI availability.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Brand {
    #[default]
    Samsung,
    Hisense,
}
impl Brand {
    pub fn label(self) -> &'static str {
        match self {
            Self::Samsung => "Samsung Tizen",
            Self::Hisense => "Hisense VIDAA",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "Action", rename_all_fields = "PascalCase")]
pub enum Action {
    Info,
    Discover,
    Connect,
    Save {
        ip_address: String,
        mac_address: String,
        certificate_path: String,
        device_uuid: String,
    },
    Pair {
        ip_address: String,
        mac_address: String,
        pin: String,
    },
    RequestPin,
    Key {
        key: String,
    },
    Wake,
    Macro {
        sequence: String,
    },
    PictureRead,
    PictureSet {
        menu_id: i32,
        value: i32,
    },
    VolumeRead,
    VolumeSet {
        value: i32,
    },
    Platform {
        name: String,
        value: i32,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Command {
    pub brand: Brand,
    #[serde(flatten)]
    pub action: Action,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Config {
    pub ip_address: String,
    pub mac_address: String,
    pub name: String,
    pub paired: bool,
    pub certificate_path: String,
    pub device_uuid: String,
    pub controller_mac: String,
    pub r#macro: String,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Device {
    pub ip_address: String,
    pub mac_address: String,
    pub name: String,
    pub model_name: String,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PictureSetting {
    pub menu_id: i32,
    pub name: String,
    pub value: String,
    pub value_type: String,
    pub flag: Option<i32>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct State {
    pub samsung: Config,
    pub hisense: Config,
    pub devices: Vec<Device>,
    pub picture_settings: Vec<PictureSetting>,
    pub volume: Option<i32>,
    pub note: Option<String>,
}
impl State {
    pub fn config(&self, brand: Brand) -> &Config {
        match brand {
            Brand::Samsung => &self.samsung,
            Brand::Hisense => &self.hisense,
        }
    }
}
