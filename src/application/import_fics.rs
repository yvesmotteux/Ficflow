use crate::domain::fanfiction::{FanfictionFetcher, FanfictionOps};
use crate::error::FicflowError;

pub enum ImportFrom<'a> {
    Ids(&'a [u64]),
    List(&'a str),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ImportPlan {
    TooMany { count: Option<usize> },
    Add { new_ids: Vec<u64>, already: usize },
}

pub fn plan_import(
    fetcher: &dyn FanfictionFetcher,
    fanfiction_ops: &dyn FanfictionOps,
    from: ImportFrom<'_>,
    limit: Option<usize>,
) -> Result<ImportPlan, FicflowError> {
    let ids = match from {
        ImportFrom::Ids(ids) => ids.to_vec(),
        ImportFrom::List(path) => {
            let list = fetcher.fetch_work_ids(path, limit.unwrap_or(usize::MAX))?;
            if list.ids.is_empty() {
                return Err(FicflowError::EmptyList);
            }
            if let Some(limit) = limit
                && list.total.unwrap_or(list.ids.len()) > limit
            {
                return Ok(ImportPlan::TooMany { count: list.total });
            }
            list.ids
        }
    };
    let mut new_ids = Vec::new();
    for id in &ids {
        match fanfiction_ops.get_fanfiction_by_id(*id) {
            Ok(_) => {}
            Err(FicflowError::NotFound { .. }) => new_ids.push(*id),
            Err(e) => return Err(e),
        }
    }
    if limit.is_some_and(|limit| new_ids.len() > limit) {
        return Ok(ImportPlan::TooMany {
            count: Some(new_ids.len()),
        });
    }
    let already = ids.len() - new_ids.len();
    Ok(ImportPlan::Add { new_ids, already })
}
