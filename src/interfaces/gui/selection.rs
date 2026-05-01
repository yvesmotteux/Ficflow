/// Library-table selection state.
///
/// `Multi` is reserved for the multi-selection phase and not produced by the
/// current library view; the variant lives here so consumers (details panel,
/// future selection bar) already have the shape they'll need.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Selection {
    #[default]
    None,
    Single(u64),
    #[allow(dead_code)]
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
