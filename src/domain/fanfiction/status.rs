use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

#[derive(
    Clone,
    Copy,
    Debug,
    Serialize,
    Deserialize,
    Display,
    EnumString,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
)]
pub enum ReadingStatus {
    InProgress,
    Read,
    PlanToRead,
    Paused,
    Abandoned,
}

impl ReadingStatus {
    pub const ALL: [ReadingStatus; 5] = [
        ReadingStatus::InProgress,
        ReadingStatus::Read,
        ReadingStatus::PlanToRead,
        ReadingStatus::Paused,
        ReadingStatus::Abandoned,
    ];
}
