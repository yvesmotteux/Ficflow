use crate::domain::fanfiction::{Fanfiction, FanfictionOps};
use crate::error::FicflowError;

pub fn update_personal_note(
    fanfiction_ops: &dyn FanfictionOps,
    fic_id: u64,
    note_text: Option<&str>,
) -> Result<Fanfiction, FicflowError> {
    let mut fanfiction = fanfiction_ops.get_fanfiction_by_id(fic_id)?;
    fanfiction.personal_note = note_text.map(|s| s.to_string());
    fanfiction_ops.save_fanfiction(&fanfiction)?;
    Ok(fanfiction)
}
