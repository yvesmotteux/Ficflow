use crate::domain::fanfiction::{DatabaseOps, ReadingStatus};
use std::error::Error;

pub fn update_reading_status(
    db_ops: &dyn DatabaseOps,
    fic_id: u64,
    new_status: &str
) -> Result<(), Box<dyn Error>> {
    // Get the current fanfiction
    let mut fic = db_ops.get_fanfiction_by_id(fic_id)?;
    
    // Parse the status string
    let reading_status = match new_status.to_lowercase().as_str() {
        "inprogress" | "in-progress" | "in_progress" | "reading" => ReadingStatus::InProgress,
        "read" | "finished" | "completed" => ReadingStatus::Read,
        "plantoread" | "plan-to-read" | "plan_to_read" | "plan" | "ptr" | "tbr" => ReadingStatus::PlanToRead,
        "paused" => ReadingStatus::Paused,
        "abandoned" => ReadingStatus::Abandoned,
        _ => return Err(format!("Invalid reading status: '{}'. Valid options are: 'inprogress', 'read', 'plantoread', 'paused', 'abandoned'", new_status).into())
    };
    
    // Update the reading status
    fic.reading_status = reading_status;
    
    // Update the fanfiction in the database
    db_ops.update_fanfiction(&fic)?;
    
    Ok(())
}
