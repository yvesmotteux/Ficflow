use super::*;

impl FicflowApp {
    pub(super) fn paint_modals(&mut self, ctx: &egui::Context) {
        let columns_changed = column_picker::show(
            ctx,
            &mut self.show_column_picker,
            &mut self.config.visible_columns,
        );
        // Outcomes extracted into a local `ModalAction` before dispatch
        // so the action handler (which takes `&mut self`) doesn't fight
        // the modal-borrow over `self.active_modal`.
        enum ModalAction {
            None,
            Close,
            CreateShelf(String),
            SwitchToAutoShelf,
            UpsertAutoShelf {
                shelf_id: Option<u64>,
                name: String,
                criteria: AutoShelfCriteria,
            },
            RenameShelf {
                shelf_id: u64,
                new_name: String,
            },
            DeleteShelf(u64),
            DeleteFics(Vec<u64>),
            RemoveFicsFromShelf {
                ids: Vec<u64>,
                shelf_id: u64,
            },
            AddFic(String),
            RestoreBackup(PathBuf),
            Quit,
        }
        let action = match &mut self.active_modal {
            ActiveModal::None => ModalAction::None,
            ActiveModal::CreateShelf(state) => match shelf_modals::draw_create(ctx, state) {
                shelf_modals::Outcome::SubmitNormal(name) => ModalAction::CreateShelf(name),
                shelf_modals::Outcome::SwitchToAuto => ModalAction::SwitchToAutoShelf,
                shelf_modals::Outcome::Cancel => ModalAction::Close,
                shelf_modals::Outcome::None => ModalAction::None,
            },
            ActiveModal::RenameShelf(state) => match shelf_modals::draw_rename(ctx, state) {
                shelf_modals::RenameOutcome::Submit { shelf_id, new_name } => {
                    ModalAction::RenameShelf { shelf_id, new_name }
                }
                shelf_modals::RenameOutcome::Cancel => ModalAction::Close,
                shelf_modals::RenameOutcome::None => ModalAction::None,
            },
            ActiveModal::AutoShelf(state) => {
                let distinct_values = auto_shelf::build_distinct_values(&self.cache.fics);
                match shelf_modals::draw_auto_shelf(ctx, state, &distinct_values) {
                    shelf_modals::AutoShelfOutcome::Submit {
                        shelf_id,
                        name,
                        criteria,
                    } => ModalAction::UpsertAutoShelf {
                        shelf_id,
                        name,
                        criteria,
                    },
                    shelf_modals::AutoShelfOutcome::Cancel => ModalAction::Close,
                    shelf_modals::AutoShelfOutcome::None => ModalAction::None,
                }
            }
            ActiveModal::DeleteShelf(id) => {
                match shelf_modals::draw_delete_confirm(ctx, *id, &self.cache.shelves) {
                    shelf_modals::DeleteOutcome::Confirm(id) => ModalAction::DeleteShelf(id),
                    shelf_modals::DeleteOutcome::Cancel => ModalAction::Close,
                    shelf_modals::DeleteOutcome::None => ModalAction::None,
                }
            }
            ActiveModal::DeleteFics(ids) => {
                match bulk_modals::draw_delete_confirm(ctx, ids, &self.cache.fics) {
                    bulk_modals::DeleteOutcome::Confirm(ids) => ModalAction::DeleteFics(ids),
                    bulk_modals::DeleteOutcome::Cancel => ModalAction::Close,
                    bulk_modals::DeleteOutcome::None => ModalAction::None,
                }
            }
            ActiveModal::RemoveOrDeleteFics { ids, shelf_id } => {
                let shelf_id = *shelf_id;
                let shelf_name = self
                    .cache
                    .shelf(shelf_id)
                    .map(|s| s.name.as_str())
                    .unwrap_or("shelf");
                match bulk_modals::draw_remove_or_delete(ctx, ids, shelf_name, &self.cache.fics) {
                    bulk_modals::RemoveOrDeleteOutcome::RemoveFromShelf(ids) => {
                        ModalAction::RemoveFicsFromShelf { ids, shelf_id }
                    }
                    bulk_modals::RemoveOrDeleteOutcome::DeleteEverywhere(ids) => {
                        ModalAction::DeleteFics(ids)
                    }
                    bulk_modals::RemoveOrDeleteOutcome::Cancel => ModalAction::Close,
                    bulk_modals::RemoveOrDeleteOutcome::None => ModalAction::None,
                }
            }
            ActiveModal::AddFic(state) => match add_fic_dialog::draw(ctx, state) {
                add_fic_dialog::Outcome::Submit(input) => ModalAction::AddFic(input),
                add_fic_dialog::Outcome::Cancel => ModalAction::Close,
                add_fic_dialog::Outcome::None => ModalAction::None,
            },
            ActiveModal::ConfirmQuit => {
                match quit_modal::draw_confirm(ctx, self.task_executor.running_count()) {
                    quit_modal::Outcome::Quit => ModalAction::Quit,
                    quit_modal::Outcome::Cancel => ModalAction::Close,
                    quit_modal::Outcome::None => ModalAction::None,
                }
            }
            ActiveModal::ConfirmRestore(backup) => match restore_modal::draw_confirm(ctx, backup) {
                restore_modal::Outcome::Confirm => ModalAction::RestoreBackup(backup.clone()),
                restore_modal::Outcome::Cancel => ModalAction::Close,
                restore_modal::Outcome::None => ModalAction::None,
            },
        };
        match action {
            ModalAction::None => {}
            ModalAction::Close => self.active_modal = ActiveModal::None,
            ModalAction::CreateShelf(name) => {
                let _ = self.create_shelf(name);
                self.active_modal = ActiveModal::None;
            }
            ModalAction::SwitchToAutoShelf => {
                self.active_modal = ActiveModal::AutoShelf(AutoShelfState::new());
            }
            ModalAction::UpsertAutoShelf {
                shelf_id,
                name,
                criteria,
            } => {
                let _ = self.upsert_auto_shelf(shelf_id, name, criteria);
                self.active_modal = ActiveModal::None;
            }
            ModalAction::RenameShelf { shelf_id, new_name } => {
                let _ = self.rename_shelf(shelf_id, new_name);
                self.active_modal = ActiveModal::None;
            }
            ModalAction::DeleteShelf(id) => {
                let _ = self.delete_shelf(id);
                self.active_modal = ActiveModal::None;
            }
            ModalAction::DeleteFics(ids) => {
                self.handle_bulk_delete(&ids);
                self.active_modal = ActiveModal::None;
            }
            ModalAction::RemoveFicsFromShelf { ids, shelf_id } => {
                let (succeeded, failed) = self.bulk_remove_from_shelf(&ids, shelf_id);
                self.toast_bulk_result("Removed from shelf", succeeded, failed);
                self.active_modal = ActiveModal::None;
            }
            ModalAction::AddFic(input) => {
                self.task_executor.enqueue_add(input);
                self.active_modal = ActiveModal::None;
            }
            ModalAction::RestoreBackup(backup) => {
                self.restore_library_backup(backup);
                self.active_modal = ActiveModal::None;
            }
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
