//! Headless GUI integration tests. These tests drive `FicflowApp`
//! frame-by-frame against a real (per-test, temp-file-backed) SQLite
//! database and an `httpmock`'d AO3 server, exercising the full
//! application stack — domain → application → infrastructure →
//! interfaces — through the same code paths a user clicks.
//!
//! `egui_kittest` would be the ideal harness, but it has no release
//! that targets egui 0.29 (earliest is 0.30). We're pinned to 0.29 for
//! the chrome work that ships in Phase 12, so the tests build a small
//! custom harness on top of plain `egui::Context::run` instead — see
//! `tests/gui/harness.rs`.

#[path = "common/mod.rs"]
mod common;

#[path = "gui/harness.rs"]
mod harness;

#[path = "gui/smoke.rs"]
mod smoke;

#[path = "gui/library.rs"]
mod library;

#[path = "gui/field_updates.rs"]
mod field_updates;

#[path = "gui/shelves.rs"]
mod shelves;

#[path = "gui/bulk.rs"]
mod bulk;

#[path = "gui/soft_delete.rs"]
mod soft_delete;

#[path = "gui/view_state.rs"]
mod view_state;
