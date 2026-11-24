use serde::{Deserialize, Serialize};

use crate::domain::fanfiction::{Fanfiction, ReadingStatus};

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

impl AutoShelfCriteria {
    /// An empty clause list matches no fics, under either AND or OR, so an
    /// auto-shelf never accidentally includes the whole library before its
    /// first clause is added.
    pub fn matches(&self, fic: &Fanfiction) -> bool {
        if self.clauses.is_empty() {
            return false;
        }
        match self.logic {
            ClauseLogic::And => self.clauses.iter().all(|c| c.matches(fic)),
            ClauseLogic::Or => self.clauses.iter().any(|c| c.matches(fic)),
        }
    }
}

impl Clause {
    fn matches(&self, fic: &Fanfiction) -> bool {
        match self {
            Clause::Tag(v) => contains_ci(fic.tags.as_deref(), v),
            Clause::Fandom(v) => contains_ci(Some(&fic.fandoms), v),
            Clause::Relationship(v) => contains_ci(fic.relationships.as_deref(), v),
            Clause::Character(v) => contains_ci(fic.characters.as_deref(), v),
            Clause::Author(v) => contains_ci(Some(&fic.authors), v),
            Clause::Status(status) => fic.reading_status == *status,
        }
    }
}

fn contains_ci(values: Option<&[String]>, needle: &str) -> bool {
    values
        .unwrap_or(&[])
        .iter()
        .any(|v| v.eq_ignore_ascii_case(needle))
}
