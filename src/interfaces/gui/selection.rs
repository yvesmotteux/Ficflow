#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Selection {
    #[default]
    None,
    Single(u64),
    Multi(Vec<u64>),
}

impl Selection {
    pub fn contains(&self, id: u64) -> bool {
        match self {
            Selection::None => false,
            Selection::Single(selected) => *selected == id,
            Selection::Multi(ids) => ids.contains(&id),
        }
    }
}

impl From<Vec<u64>> for Selection {
    fn from(ids: Vec<u64>) -> Self {
        match ids.len() {
            0 => Selection::None,
            1 => Selection::Single(ids[0]),
            _ => Selection::Multi(ids),
        }
    }
}
