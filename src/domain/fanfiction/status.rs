use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Display, PartialEq, Eq)]
pub enum ReadingStatus {
    InProgress,
    Read,
    PlanToRead,
    Paused,
    Abandoned,
}
