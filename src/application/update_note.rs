use crate::domain::fanfiction::repository::DatabaseOps;

pub fn update_personal_note(db_ops: &dyn DatabaseOps, fic_id: u64, note_text: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    // Get existing fanfiction
    let mut fanfiction = db_ops.get_fanfiction_by_id(fic_id)?;
    
    // Update the personal note
    fanfiction.personal_note = note_text.map(|s| s.to_string());
    
    // Save the updated fanfiction
    db_ops.update_fanfiction(&fanfiction)?;
    
    Ok(())
}
