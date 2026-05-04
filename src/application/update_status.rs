use crate::domain::fanfiction::{Fanfiction, FanfictionOps, ReadingStatus};
use crate::error::FicflowError;

/// Parses a CLI string into a typed `ReadingStatus`. Accepts the
/// canonical form (`"inprogress"`, `"read"`, …) plus a small set of
/// aliases (`"reading"`, `"finished"`, `"plan"`, `"ptr"`, `"tbr"`, …)
/// so users can type what feels natural. Used by the CLI argument
/// path; GUI callers already hold a typed `ReadingStatus`.
pub fn parse_reading_status(input: &str) -> Result<ReadingStatus, FicflowError> {
    Ok(match input.to_lowercase().as_str() {
        "inprogress" | "in-progress" | "in_progress" | "reading" => ReadingStatus::InProgress,
        "read" | "finished" | "completed" => ReadingStatus::Read,
        "plantoread" | "plan-to-read" | "plan_to_read" | "plan" | "ptr" | "tbr" => {
            ReadingStatus::PlanToRead
        }
        "paused" => ReadingStatus::Paused,
        "abandoned" => ReadingStatus::Abandoned,
        _ => {
            return Err(FicflowError::InvalidInput(format!(
                "Invalid reading status: '{}'. Valid options are: 'inprogress', 'read', 'plantoread', 'paused', 'abandoned'",
                input
            )));
        }
    })
}

pub fn update_reading_status(
    fanfiction_ops: &dyn FanfictionOps,
    fic_id: u64,
    new_status: ReadingStatus,
) -> Result<Fanfiction, FicflowError> {
    let mut fic = fanfiction_ops.get_fanfiction_by_id(fic_id)?;
    fic.reading_status = new_status;
    fanfiction_ops.save_fanfiction(&fic)?;

    Ok(fic)
}
