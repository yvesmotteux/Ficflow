use serde::{Serialize, Deserialize};
use strum_macros::Display;

#[derive(Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum ReadingStatus {
    InProgress,
    Read,
    PlanToRead,
    Paused,
    Abandoned,
}
