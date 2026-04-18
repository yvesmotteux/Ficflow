use crate::domain::fanfiction::{DatabaseOps, Fanfiction, ReadingStatus};
use crate::error::FicflowError;

pub fn update_reading_status(
    db_ops: &dyn DatabaseOps,
    fic_id: u64,
    new_status: &str,
) -> Result<Fanfiction, FicflowError> {
    let mut fic = db_ops.get_fanfiction_by_id(fic_id)?;

    let reading_status = match new_status.to_lowercase().as_str() {
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
                new_status
            )));
        }
    };

    fic.reading_status = reading_status;
    db_ops.save_fanfiction(&fic)?;

    Ok(fic)
}
