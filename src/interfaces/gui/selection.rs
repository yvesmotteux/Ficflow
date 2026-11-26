#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Selection {
    #[default]
    None,
    Single(u64),
    Multi(Vec<u64>),
    Series(u64),
    Missing {
        series_id: u64,
        work_id: u64,
    },
    Hidden {
        series_id: u64,
        part: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RowKey {
    Fic(u64),
    Series(u64),
    Missing { series_id: u64, work_id: u64 },
    Hidden { series_id: u64, part: u32 },
}

impl Selection {
    pub fn contains(&self, id: u64) -> bool {
        match self {
            Selection::Single(selected) => *selected == id,
            Selection::Multi(ids) => ids.contains(&id),
            _ => false,
        }
    }

    pub fn shows(&self, key: RowKey) -> bool {
        match key {
            RowKey::Fic(id) => self.contains(id),
            RowKey::Series(id) => *self == Selection::Series(id),
            RowKey::Missing { series_id, work_id } => {
                *self == Selection::Missing { series_id, work_id }
            }
            RowKey::Hidden { series_id, part } => *self == Selection::Hidden { series_id, part },
        }
    }
}

impl From<RowKey> for Selection {
    fn from(key: RowKey) -> Self {
        match key {
            RowKey::Fic(id) => Selection::Single(id),
            RowKey::Series(id) => Selection::Series(id),
            RowKey::Missing { series_id, work_id } => Selection::Missing { series_id, work_id },
            RowKey::Hidden { series_id, part } => Selection::Hidden { series_id, part },
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
