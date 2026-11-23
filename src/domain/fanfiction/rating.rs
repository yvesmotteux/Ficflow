use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum UserRating {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
}

impl TryFrom<u8> for UserRating {
    type Error = u8;

    fn try_from(n: u8) -> Result<Self, u8> {
        match n {
            1 => Ok(UserRating::One),
            2 => Ok(UserRating::Two),
            3 => Ok(UserRating::Three),
            4 => Ok(UserRating::Four),
            5 => Ok(UserRating::Five),
            other => Err(other),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Display, EnumString, PartialEq)]
pub enum Rating {
    NotRated,
    General,
    TeenAndUp,
    Mature,
    Explicit,
}

#[derive(Clone, Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum ArchiveWarnings {
    ChooseNotToUse,
    GraphicDepictionsOfViolence,
    MajorCharacterDeath,
    NoArchiveWarningsApply,
    RapeNonCon,
    Underage,
}

#[derive(Clone, Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum Categories {
    FF,
    FM,
    MM,
    Gen,
    Other,
    Multi,
}
