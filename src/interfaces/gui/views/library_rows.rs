use std::collections::{HashMap, HashSet};

use crate::domain::fanfiction::Fanfiction;
use crate::domain::series::Series;

use super::super::config::SortPref;
use super::super::selection::RowKey;
use super::super::view::View;
use super::library_view::{filter_and_sort, matches_search};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryRow {
    Series {
        series: usize,
        local: usize,
        total: u32,
        collapsed: bool,
    },
    Fic {
        fic: usize,
        part: Option<(u32, u32)>,
    },
    Missing {
        series: usize,
        work: usize,
        total: u32,
    },
}

impl LibraryRow {
    pub fn key(&self, fics: &[Fanfiction], series: &[Series]) -> RowKey {
        match *self {
            LibraryRow::Series { series: s, .. } => RowKey::Series(series[s].id),
            LibraryRow::Fic { fic, .. } => RowKey::Fic(fics[fic].id),
            LibraryRow::Missing {
                series: s, work, ..
            } => RowKey::Missing {
                series_id: series[s].id,
                work_id: series[s].works[work].work_id,
            },
        }
    }
}

pub struct RowInputs<'a> {
    pub fics: &'a [Fanfiction],
    pub series: &'a [Series],
    pub view: &'a View,
    pub query: &'a str,
    pub sort: SortPref,
    pub collapsed: &'a [u64],
    pub shelf_members: &'a HashSet<u64>,
}

struct Group {
    series: usize,
    rank: usize,
    members: Vec<(u32, usize)>,
    missing: Vec<(u32, usize)>,
    local: usize,
    total: u32,
}

pub fn build_rows(input: &RowInputs) -> Vec<LibraryRow> {
    let RowInputs {
        fics,
        series,
        view,
        query,
        sort,
        collapsed,
        shelf_members,
    } = *input;
    let query = query.trim().to_lowercase();
    let in_view = filter_and_sort(fics, "", sort, view, shelf_members);
    let rank: HashMap<usize, usize> = in_view.iter().enumerate().map(|(r, &i)| (i, r)).collect();
    let fic_index: HashMap<u64, usize> = fics.iter().enumerate().map(|(i, f)| (f.id, i)).collect();
    let matches = |i: usize| rank.contains_key(&i) && matches_search(&fics[i], &query);

    let mut groups: Vec<Group> = Vec::new();
    for (s, series) in series.iter().enumerate() {
        let local: Vec<(u32, usize)> = series
            .works
            .iter()
            .filter_map(|w| fic_index.get(&w.work_id).map(|&i| (w.part, i)))
            .collect();
        if local.is_empty() {
            continue;
        }
        let title_match = !query.is_empty() && series.title.to_lowercase().contains(&query);
        let members: Vec<(u32, usize)> = local
            .iter()
            .copied()
            .filter(|&(_, i)| {
                if title_match {
                    rank.contains_key(&i)
                } else {
                    matches(i)
                }
            })
            .collect();
        let whole_shelf = !matches!(view, View::Shelf(_)) || members.len() == local.len();
        let grouped = match () {
            _ if members.is_empty() || !whole_shelf => false,
            _ if query.is_empty() || title_match => true,
            _ => members.len() >= 2,
        };
        if !grouped {
            continue;
        }
        let show_missing =
            matches!(view, View::AllFics | View::Shelf(_)) && (query.is_empty() || title_match);
        let missing = if show_missing {
            series
                .works
                .iter()
                .enumerate()
                .filter(|(_, w)| !fic_index.contains_key(&w.work_id))
                .map(|(w, work)| (work.part, w))
                .collect()
        } else {
            Vec::new()
        };
        let max_part = series.works.iter().map(|w| w.part).max().unwrap_or(0);
        groups.push(Group {
            series: s,
            rank: members
                .iter()
                .map(|(_, i)| rank[i])
                .min()
                .unwrap_or(usize::MAX),
            members,
            missing,
            local: local.len(),
            total: series.works_count.max(max_part),
        });
    }

    let grouped: HashSet<usize> = groups
        .iter()
        .flat_map(|g| g.members.iter().map(|&(_, i)| i))
        .collect();
    enum Entry<'g> {
        Fic(usize),
        Group(&'g Group),
    }
    let mut entries: Vec<(usize, Entry)> = in_view
        .iter()
        .copied()
        .filter(|&i| !grouped.contains(&i) && matches(i))
        .map(|i| (rank[&i], Entry::Fic(i)))
        .chain(groups.iter().map(|g| (g.rank, Entry::Group(g))))
        .collect();
    entries.sort_by_key(|(r, entry)| (*r, matches!(entry, Entry::Fic(_))));

    let mut rows = Vec::new();
    for (_, entry) in entries {
        match entry {
            Entry::Fic(fic) => rows.push(LibraryRow::Fic { fic, part: None }),
            Entry::Group(group) => {
                let is_collapsed = collapsed.contains(&series[group.series].id);
                rows.push(LibraryRow::Series {
                    series: group.series,
                    local: group.local,
                    total: group.total,
                    collapsed: is_collapsed,
                });
                if is_collapsed {
                    continue;
                }
                let mut children: Vec<(u32, LibraryRow)> = group
                    .members
                    .iter()
                    .map(|&(part, fic)| {
                        (
                            part,
                            LibraryRow::Fic {
                                fic,
                                part: Some((part, group.total)),
                            },
                        )
                    })
                    .chain(group.missing.iter().map(|&(part, work)| {
                        (
                            part,
                            LibraryRow::Missing {
                                series: group.series,
                                work,
                                total: group.total,
                            },
                        )
                    }))
                    .collect();
                children.sort_by_key(|(part, _)| *part);
                rows.extend(children.into_iter().map(|(_, row)| row));
            }
        }
    }
    rows
}

pub fn fic_ids(rows: &[LibraryRow], fics: &[Fanfiction]) -> Vec<u64> {
    let mut seen = HashSet::new();
    rows.iter()
        .filter_map(|row| match row {
            LibraryRow::Fic { fic, .. } => Some(fics[*fic].id),
            _ => None,
        })
        .filter(|id| seen.insert(*id))
        .collect()
}

pub fn series_fic_ids(series: &Series, fics: &[Fanfiction]) -> Vec<u64> {
    let local: HashSet<u64> = fics.iter().map(|f| f.id).collect();
    series
        .works
        .iter()
        .map(|w| w.work_id)
        .filter(|id| local.contains(id))
        .collect()
}
