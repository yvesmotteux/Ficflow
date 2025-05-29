use serde::{Serialize, Deserialize};
use strum_macros::Display;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum UserRating {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
}

#[derive(Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum Rating {
    NotRated,
    General,
    TeenAndUp,
    Mature,
    Explicit,
}

#[derive(Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum ArchiveWarnings {
    ChooseNotToUse,
    GraphicDepictionsOfViolence,
    MajorCharacterDeath,
    NoArchiveWarningsApply,
    RapeNonCon,
    Underage,
}

#[derive(Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum Categories {
    FF,
    FM,
    MM,
    Gen,
    Other,
    Multi,
}
