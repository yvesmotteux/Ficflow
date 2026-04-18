use crate::domain::fanfiction::{DatabaseOps, Fanfiction};
use crate::error::FicflowError;

pub fn update_personal_note(
    db_ops: &dyn DatabaseOps,
    fic_id: u64,
    note_text: Option<&str>,
) -> Result<Fanfiction, FicflowError> {
    let mut fanfiction = db_ops.get_fanfiction_by_id(fic_id)?;
    fanfiction.personal_note = note_text.map(|s| s.to_string());
    db_ops.save_fanfiction(&fanfiction)?;
    Ok(fanfiction)
}
