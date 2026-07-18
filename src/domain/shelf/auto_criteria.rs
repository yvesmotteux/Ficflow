use serde::{Deserialize, Serialize};

use crate::domain::fanfiction::ReadingStatus;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShelfKind {
    Normal,
    Auto(AutoShelfCriteria),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AutoShelfCriteria {
    pub logic: ClauseLogic,
    pub clauses: Vec<Clause>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ClauseLogic {
    #[default]
    And,
    Or,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Clause {
    Tag(String),
    Fandom(String),
    Relationship(String),
    Character(String),
    Author(String),
    Status(ReadingStatus),
}
