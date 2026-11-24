use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Display, EnumString, PartialEq, Eq)]
pub enum ReadingStatus {
    InProgress,
    Read,
    PlanToRead,
    Paused,
    Abandoned,
}
