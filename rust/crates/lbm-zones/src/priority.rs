//! Process priority classes — port of `Engine/Priority.h` + `GetPriority`.

/// C++ `enum Priority {Idle, Below, Normal, Above, High, Realtime}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Priority {
    Idle = 0,
    Below = 1,
    Normal = 2,
    Above = 3,
    High = 4,
    Realtime = 5,
}

impl Priority {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Priority {
        match v {
            0 => Priority::Idle,
            1 => Priority::Below,
            3 => Priority::Above,
            4 => Priority::High,
            5 => Priority::Realtime,
            _ => Priority::Normal,
        }
    }

    /// C++ `GetPriority(const std::string&)`: unknown -> Normal.
    pub fn parse(name: &str) -> Priority {
        match name {
            "Idle" => Priority::Idle,
            "Below" => Priority::Below,
            "Normal" => Priority::Normal,
            "Above" => Priority::Above,
            "High" => Priority::High,
            "Realtime" => Priority::Realtime,
            _ => Priority::Normal,
        }
    }
}
