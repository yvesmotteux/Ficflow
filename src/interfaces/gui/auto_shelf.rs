use std::collections::BTreeSet;

use crate::domain::fanfiction::Fanfiction;

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
