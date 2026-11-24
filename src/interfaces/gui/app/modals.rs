use super::*;
use crate::interfaces::gui::views::modals::ModalOutcome;
use crate::interfaces::gui::views::modals::bulk_modals::RemoveOrDelete;
use crate::interfaces::gui::views::modals::shelf_modals::{AutoShelfSubmit, CreateChoice};

// Outcomes are extracted into a `ModalAction` before dispatch so the
// action handler (which takes `&mut self`) doesn't fight the modal
// borrow over `self.active_modal`.
enum ModalAction {
    None,
    Close,
    CreateShelf(String),
    SwitchToAutoShelf,
    UpsertAutoShelf(AutoShelfSubmit),
    RenameShelf { shelf_id: u64, new_name: String },
    DeleteShelf(u64),
    DeleteFics(Vec<u64>),
    RemoveFicsFromShelf { ids: Vec<u64>, shelf_id: u64 },
    AddFic(String),
    RestoreBackup(PathBuf),
    Quit,
}

fn on_submit<T>(outcome: ModalOutcome<T>, action: impl FnOnce(T) -> ModalAction) -> ModalAction {
    match outcome {
        ModalOutcome::None => ModalAction::None,
        ModalOutcome::Submit(value) => action(value),
        ModalOutcome::Cancel => ModalAction::Close,
    }
}

impl FicflowApp {
    pub(super) fn paint_modals(&mut self, ctx: &egui::Context) {
        let columns_changed = column_picker::show(
            ctx,
            &mut self.show_column_picker,
            &mut self.config.visible_columns,
        );
        let action = match &mut self.active_modal {
            ActiveModal::None => ModalAction::None,
            ActiveModal::CreateShelf(state) => on_submit(
                shelf_modals::draw_create(ctx, state),
                |choice| match choice {
                    CreateChoice::Normal(name) => ModalAction::CreateShelf(name),
                    CreateChoice::Auto => ModalAction::SwitchToAutoShelf,
                },
            ),
            ActiveModal::RenameShelf(state) => {
                let shelf_id = state.shelf_id;
                on_submit(shelf_modals::draw_rename(ctx, state), |new_name| {
                    ModalAction::RenameShelf { shelf_id, new_name }
                })
            }
            ActiveModal::AutoShelf(state) => {
                let distinct_values = auto_shelf::build_distinct_values(&self.cache.fics);
                on_submit(
                    shelf_modals::draw_auto_shelf(ctx, state, &distinct_values),
                    ModalAction::UpsertAutoShelf,
                )
            }
            ActiveModal::DeleteShelf(id) => {
                let id = *id;
                on_submit(
                    shelf_modals::draw_delete_confirm(ctx, id, &self.cache.shelves),
                    |()| ModalAction::DeleteShelf(id),
                )
            }
            ActiveModal::DeleteFics(ids) => on_submit(
                bulk_modals::draw_delete_confirm(ctx, ids, &self.cache.fics),
                |()| ModalAction::DeleteFics(ids.clone()),
            ),
            ActiveModal::RemoveOrDeleteFics { ids, shelf_id } => {
                let shelf_id = *shelf_id;
                let shelf_name = self.cache.shelf(shelf_id).map_or("shelf", |s| &s.name);
                on_submit(
                    bulk_modals::draw_remove_or_delete(ctx, ids, shelf_name, &self.cache.fics),
                    |choice| match choice {
                        RemoveOrDelete::RemoveFromShelf => ModalAction::RemoveFicsFromShelf {
                            ids: ids.clone(),
                            shelf_id,
                        },
                        RemoveOrDelete::DeleteEverywhere => ModalAction::DeleteFics(ids.clone()),
                    },
                )
            }
            ActiveModal::AddFic(state) => {
                on_submit(add_fic_dialog::draw(ctx, state), ModalAction::AddFic)
            }
            ActiveModal::ConfirmQuit => on_submit(
                quit_modal::draw_confirm(ctx, self.task_executor.running_count()),
                |()| ModalAction::Quit,
            ),
            ActiveModal::ConfirmRestore(backup) => {
                on_submit(restore_modal::draw_confirm(ctx, backup), |()| {
                    ModalAction::RestoreBackup(backup.clone())
                })
            }
        };
        if !matches!(action, ModalAction::None) {
            self.active_modal = ActiveModal::None;
        }
        match action {
            ModalAction::None | ModalAction::Close => {}
            ModalAction::CreateShelf(name) => {
                let _ = self.create_shelf(name);
            }
            ModalAction::SwitchToAutoShelf => {
                self.active_modal = ActiveModal::AutoShelf(AutoShelfState::new());
            }
            ModalAction::UpsertAutoShelf(submit) => {
                let _ = self.upsert_auto_shelf(submit.shelf_id, submit.name, submit.criteria);
            }
            ModalAction::RenameShelf { shelf_id, new_name } => {
                let _ = self.rename_shelf(shelf_id, new_name);
            }
            ModalAction::DeleteShelf(id) => {
                let _ = self.delete_shelf(id);
            }
            ModalAction::DeleteFics(ids) => self.handle_bulk_delete(&ids),
            ModalAction::RemoveFicsFromShelf { ids, shelf_id } => {
                let (succeeded, failed) = self.bulk_remove_from_shelf(&ids, shelf_id);
                self.toast_bulk_result("Removed from shelf", succeeded, failed);
            }
            ModalAction::AddFic(input) => self.task_executor.enqueue_add(input),
            ModalAction::RestoreBackup(backup) => self.restore_library_backup(backup),
            ModalAction::Quit => {
                self.confirm_quit();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if columns_changed {
            self.save_config();
        }
    }
}
