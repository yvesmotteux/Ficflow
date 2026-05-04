//! Floating windows opened on demand from the main UI: column picker,
//! add-fic input, shelf create/delete confirmations, and the bulk-delete
//! confirmation. They share no internal state with the views that own
//! their open/closed flags — the parent passes `&mut state` and the
//! modal returns an Outcome enum the parent dispatches on.

pub mod add_fic_dialog;
pub mod bulk_modals;
pub mod column_picker;
pub mod shelf_modals;
