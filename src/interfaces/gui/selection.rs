/// Library-table selection state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Selection {
    #[default]
    None,
    Single(u64),
    Multi(Vec<u64>),
}

impl Selection {
    /// True when the given fic id is part of the current selection.
    pub fn contains(&self, id: u64) -> bool {
        match self {
            Selection::None => false,
            Selection::Single(selected) => *selected == id,
            Selection::Multi(ids) => ids.contains(&id),
        }
    }
}

/// Single source of truth for "build a Selection from a list of ids".
/// Variant is derived from length: empty → `None`, one → `Single`, more
/// → `Multi`. Consumers (range selection, ctrl-click toggle, prune-to-
/// view, programmatic select-many) all funnel through here so the
/// invariant lives in one place.
impl From<Vec<u64>> for Selection {
    fn from(ids: Vec<u64>) -> Self {
        match ids.len() {
            0 => Selection::None,
            1 => Selection::Single(ids[0]),
            _ => Selection::Multi(ids),
        }
    }
}
