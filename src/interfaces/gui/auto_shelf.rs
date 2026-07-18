use std::collections::{BTreeSet, HashSet};

use crate::domain::fanfiction::Fanfiction;
use crate::domain::shelf::{AutoShelfCriteria, Clause, ClauseLogic};

fn contains_ci(values: Option<&[String]>, needle: &str) -> bool {
    values
        .unwrap_or(&[])
        .iter()
        .any(|v| v.eq_ignore_ascii_case(needle))
}

fn eval_clause(fic: &Fanfiction, clause: &Clause) -> bool {
    match clause {
        Clause::Tag(v) => contains_ci(fic.tags.as_deref(), v),
        Clause::Fandom(v) => contains_ci(Some(&fic.fandoms), v),
        Clause::Relationship(v) => contains_ci(fic.relationships.as_deref(), v),
        Clause::Character(v) => contains_ci(fic.characters.as_deref(), v),
        Clause::Author(v) => contains_ci(Some(&fic.authors), v),
        Clause::Status(status) => fic.reading_status == *status,
    }
}

/// Whether `fic` satisfies `criteria`. An empty clause list matches no
/// fics, under either AND or OR, so an auto-shelf never accidentally
/// includes the whole library before its first clause is added.
pub fn matches(fic: &Fanfiction, criteria: &AutoShelfCriteria) -> bool {
    if criteria.clauses.is_empty() {
        return false;
    }
    match criteria.logic {
        ClauseLogic::And => criteria.clauses.iter().all(|c| eval_clause(fic, c)),
        ClauseLogic::Or => criteria.clauses.iter().any(|c| eval_clause(fic, c)),
    }
}

pub fn matching_fic_ids(fics: &[Fanfiction], criteria: &AutoShelfCriteria) -> HashSet<u64> {
    fics.iter()
        .filter(|f| matches(f, criteria))
        .map(|f| f.id)
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct DistinctValues {
    pub tags: Vec<String>,
    pub fandoms: Vec<String>,
    pub relationships: Vec<String>,
    pub characters: Vec<String>,
    pub authors: Vec<String>,
}

pub fn build_distinct_values(fics: &[Fanfiction]) -> DistinctValues {
    let mut tags = BTreeSet::new();
    let mut fandoms = BTreeSet::new();
    let mut relationships = BTreeSet::new();
    let mut characters = BTreeSet::new();
    let mut authors = BTreeSet::new();

    for fic in fics {
        tags.extend(fic.tags.iter().flatten().cloned());
        fandoms.extend(fic.fandoms.iter().cloned());
        relationships.extend(fic.relationships.iter().flatten().cloned());
        characters.extend(fic.characters.iter().flatten().cloned());
        authors.extend(fic.authors.iter().cloned());
    }

    DistinctValues {
        tags: tags.into_iter().collect(),
        fandoms: fandoms.into_iter().collect(),
        relationships: relationships.into_iter().collect(),
        characters: characters.into_iter().collect(),
        authors: authors.into_iter().collect(),
    }
}
