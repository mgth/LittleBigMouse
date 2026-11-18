//! DDC/CI values shared by the agent and the property panel.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Choice {
    pub value: u16,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Feature {
    pub code: u8,
    pub value: u16,
    /// Lowest level of a continuous control. Above 0 when 0 means something else.
    #[serde(default)]
    pub minimum: u16,
    pub maximum: u16,
    /// The values a non-continuous control can take, or the special values that a
    /// continuous one has outside its range.
    pub choices: Vec<Choice>,
}

impl Feature {
    pub fn accepts(&self, value: u16) -> bool {
        if continuous(self.code) {
            (self.minimum..=self.maximum).contains(&value)
                || self.choices.iter().any(|c| c.value == value)
        } else {
            self.choices.iter().any(|c| c.value == value)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Monitor {
    /// A fresh probe invalidates writes based on older capabilities/values.
    pub token: u64,
    pub connection: String,
    pub features: Vec<Feature>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Write {
    pub token: u64,
    pub code: u8,
    pub value: u16,
}

// Deliberate allow-list: no resets, vendor opcodes or arbitrary hardware writes.
pub const CODES: &[u8] = &[
    0x10, 0x12, 0x14, 0x16, 0x18, 0x1a, 0x6c, 0x6e, 0x70, 0x62, 0x8d, 0x60, 0xd6,
];
pub fn continuous(code: u8) -> bool {
    matches!(
        code,
        0x10 | 0x12 | 0x16 | 0x18 | 0x1a | 0x6c | 0x6e | 0x70 | 0x62
    )
}
pub const VOLUME: u8 = 0x62;
/// MCCS 2.2 and 3.0 volume: 1 to 254 are levels, 0 and 255 are not.
pub const VOLUME_LEVELS: (u16, u16) = (1, 254);
pub fn volume_choices() -> Vec<Choice> {
    [(0, "Default level"), (255, "Muted")]
        .map(|(value, label)| Choice {
            value,
            label: label.into(),
        })
        .into()
}
pub fn name(code: u8) -> &'static str {
    match code {
        0x10 => "Brightness",
        0x12 => "Contrast",
        0x14 => "Colour preset",
        0x16 | 0x6c => "Red",
        0x18 | 0x6e => "Green",
        0x1a | 0x70 => "Blue",
        0x62 => "Volume",
        0x8d => "Mute",
        0x60 => "Input source",
        0xd6 => "Power",
        _ => "Unknown",
    }
}
