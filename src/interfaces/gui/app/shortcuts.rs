use super::*;

impl FicflowApp {
    /// Skipped while a text edit has focus so we don't hijack the user's
    /// typing (Ctrl+A in a TextEdit should select the text, not all rows).
    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // The search field only mounts in library views; a stale
        // pending-focus flag would retroactively grab focus on return.
        if !self.current_view.shows_library() {
            self.focus_search_pending = false;
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        use egui::{Key, KeyboardShortcut, Modifiers};

        let pressed_esc = ctx.input(|i| i.key_pressed(Key::Escape));
        let pressed_delete = ctx.input(|i| i.key_pressed(Key::Delete));
        let ctrl_a = ctx
            .input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::A)));
        let ctrl_n = ctx
            .input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::N)));
        let ctrl_f = ctx
            .input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::F)));

        if pressed_esc && !matches!(self.selection.current(), Selection::None) {
            self.clear_selection();
        }

        if pressed_delete {
            let selected_fics = self.selected_fic_ids();
            if !selected_fics.is_empty() {
                if let View::Shelf(shelf_id) = &self.current_view {
                    let shelf_id = *shelf_id;
                    if self.is_normal_shelf(shelf_id) {
                        self.active_modal = ActiveModal::RemoveOrDeleteFics {
                            ids: selected_fics,
                            shelf_id,
                        };
                    } else {
                        // Auto-shelf membership is derived, not stored, so
                        // there's nothing to remove from → delete-only.
                        self.active_modal = ActiveModal::DeleteFics(selected_fics);
                    }
                } else if self.current_view.shows_library() {
                    self.active_modal = ActiveModal::DeleteFics(selected_fics);
                }
            } else if let View::Shelf(shelf_id) = &self.current_view
                && matches!(self.selection.current(), Selection::None)
            {
                // No fic selection on a shelf view → Delete targets the
                // shelf itself.
                self.active_modal = ActiveModal::DeleteShelf(*shelf_id);
            }
        }

        if ctrl_a && self.current_view.shows_library() {
            let ids = library_view::visible_ids(
                &self.cache.fics,
                &self.cache.series,
                &self.current_view,
                &self.cache.shelf_members,
                &self.search_query,
                self.sort,
            );
            self.selection.select_many(&ids);
            self.refresh_selection_shelf_ids();
        }

        if ctrl_n {
            self.active_modal = ActiveModal::CreateShelf(CreateState::default());
        }

        if ctrl_f && self.current_view.shows_library() {
            self.focus_search_pending = true;
        }
    }
}
