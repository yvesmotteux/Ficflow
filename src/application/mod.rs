//! Application use-cases — one file per operation users can perform,
//! whether through the CLI or the GUI. Most files are short on
//! purpose: even when a use-case is a one-line wrapper around a
//! repository method (`count_fics_in_shelf`, `list_shelves_for_fic`,
//! `get_fic`, `list_fics`, `list_shelves`, `wipe_db`), it lives here
//! as its own file so:
//!
//!   - every operation has the same shape (`pub fn op(repo, args) ->
//!     Result<…, FicflowError>`), no matter which layer below it
//!     does the actual work;
//!   - `grep` over `application/` lists every operation the system
//!     supports, with no second-tier "but this read goes straight to
//!     the repo";
//!   - if a future requirement adds validation / orchestration / a
//!     second write to a previously-trivial read, there's a place to
//!     put it without churning callers.
//!
//! The cost is a handful of two-line wrappers. The benefit is that
//! `application/` is the one canonical surface for "what can users
//! do." We commit to the rule: every operation gets a use-case file.

pub mod add_fic;
pub mod add_to_shelf;
pub mod check_updates;
pub mod count_fics_in_shelf;
pub mod count_fics_per_shelf;
pub mod create_shelf;
pub mod delete_fic;
pub mod delete_shelf;
pub mod get_fic;
pub mod list_fics;
pub mod list_shelf_fics;
pub mod list_shelves;
pub mod list_shelves_for_fic;
pub mod remove_from_shelf;
pub mod update_chapters;
pub mod update_note;
pub mod update_rating;
pub mod update_read_count;
pub mod update_status;
pub mod wipe_db;
