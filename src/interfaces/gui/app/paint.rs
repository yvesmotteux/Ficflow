use super::*;

fn compute_library_counts(fics: &[Fanfiction]) -> LibraryCounts {
    let mut counts = LibraryCounts {
        all: fics.len(),
        ..LibraryCounts::default()
    };
    for f in fics {
        counts.by_status[f.reading_status as usize] += 1;
    }
    counts
}

impl FicflowApp {
    /// Headless-friendly entry point — eframe's `App::ui` is a thin
    /// delegate so tests can drive the app via `Context::run_ui`.
    ///
    /// The host `CentralPanel` exists because egui's top-level panels
    /// would otherwise claim from the full available rect and punch
    /// through the chrome. Nesting every panel inside a single
    /// `UiBuilder::max_rect(content_rect)` constrains them to the
    /// chrome's content area with a single anchor change.
    pub fn render(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.sync_window_state(&ctx);
        self.sync_text_zoom(&ctx);
        self.handle_close_request(&ctx);
        self.handle_shortcuts(&ctx);

        let screen = ctx.content_rect();
        self.chrome.paint_background(&ctx, screen);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let controls_rect = self.chrome.draw_window_controls(ui, screen);
                self.chrome.handle_interactions(ui, screen, controls_rect);

                let content_rect = self.chrome.content_rect(screen);
                ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |host| {
                    self.paint_header(host);
                    self.paint_sidebar(host);
                    self.paint_details_panel(host);
                    self.paint_selection_bar(host);
                    self.paint_central(host);
                });
            });
        self.paint_modals(&ctx);
        self.drain_worker_events(&ctx);
        self.draw_drag_preview(&ctx);
        self.toasts.show(&ctx);

        // A slow repaint heartbeat so hover state recovers after an
        // OS-managed move/resize — winit doesn't forward a pointer-enter
        // event back, so egui's hover bookkeeping otherwise goes stale
        // until the user wiggles the mouse. Input, animations and toasts
        // request their own repaints, so idle frames don't need to run
        // continuously.
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }

    pub(super) fn paint_header(&self, host: &mut egui::Ui) {
        egui::Panel::top("ficflow-header").show(host, |ui| {
            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("FICFLOW")
                            .family(egui::FontFamily::Name(theme::NEUE_FAMILY.into()))
                            .size(20.0)
                            .color(theme::accent(ui.visuals())),
                    )
                    .selectable(false),
                );
            });
            ui.add_space(4.0);
        });
    }

    /// Sidebar can mutate `current_view`; the prev/post diff at the
    /// end refreshes derived caches and prunes the selection.
    pub(super) fn paint_sidebar(&mut self, host: &mut egui::Ui) {
        let prev_view = self.current_view.clone();
        let library_counts = compute_library_counts(&self.cache.fics);
        let mut outcome = sidebar::Outcome::None;
        egui::Panel::left("ficflow-sidebar")
            .default_size(160.0)
            .size_range(140.0..=600.0)
            .resizable(true)
            .show(host, |ui| {
                outcome = sidebar::draw(
                    ui,
                    SidebarState {
                        current_view: &mut self.current_view,
                        shelves: &self.cache.shelves,
                        library_counts: &library_counts,
                        shelf_counts: &self.cache.shelf_counts,
                        running_tasks: self.task_executor.running_count(),
                    },
                );
            });
        match outcome {
            sidebar::Outcome::None => {}
            sidebar::Outcome::OpenCreateShelfModal => {
                self.active_modal = ActiveModal::CreateShelf(CreateState::default());
            }
            sidebar::Outcome::OpenRenameShelfModal(id) => {
                if let Some(shelf) = self.cache.shelf(id) {
                    self.active_modal = ActiveModal::RenameShelf(RenameState::new(shelf));
                }
            }
            sidebar::Outcome::OpenEditAutoShelfModal(id) => {
                if let Some(state) = self.cache.shelf(id).and_then(AutoShelfState::from_shelf) {
                    self.active_modal = ActiveModal::AutoShelf(state);
                }
            }
            sidebar::Outcome::OpenDeleteShelfConfirm(id) => {
                self.active_modal = ActiveModal::DeleteShelf(id);
            }
            sidebar::Outcome::DropOnShelf { shelf_id, fic_ids } => {
                let (succeeded, failed) = self.bulk_add_to_shelf(&fic_ids, shelf_id);
                self.toast_bulk_result("Added to shelf", succeeded, failed);
            }
            sidebar::Outcome::MoveShelf {
                shelf_id,
                new_parent,
            } => {
                let _ = self.move_shelf(shelf_id, new_parent);
            }
            sidebar::Outcome::TogglePinShelf(shelf_id) => {
                let _ = self.toggle_pin_shelf(shelf_id);
            }
        }
        if self.current_view != prev_view {
            self.refresh_shelf_members();
            self.prune_selection_to_view();
            self.persist_current_view();
        }
    }

    /// Shelves a fic can be manually added to/removed from — excludes
    /// auto-shelves, whose membership is computed from criteria and
    /// would just reject the write (same rule the sidebar's
    /// drag-and-drop already enforces for these two UI surfaces).
    pub fn assignable_shelves(&self) -> Vec<Shelf> {
        self.cache
            .shelves
            .iter()
            .filter(|s| !matches!(s.kind, ShelfKind::Auto(_)))
            .cloned()
            .collect()
    }

    pub(super) fn paint_details_panel(&mut self, host: &mut egui::Ui) {
        let Selection::Single(id) = *self.selection.current() else {
            return;
        };
        if !self.current_view.shows_library() {
            return;
        }
        // Clone so the immutable borrow on `cache.fics` releases before
        // dispatching outcomes through `&mut self`.
        let Some(fic) = self.cache.fic(id).cloned() else {
            return;
        };

        let assignable_shelves = self.assignable_shelves();
        let mut outcome = details_panel::Outcome::None;
        egui::Panel::right("ficflow-details-v2")
            .default_size(320.0)
            .size_range(280.0..=900.0)
            .resizable(true)
            .show(host, |ui| {
                outcome = details_panel::draw(
                    ui,
                    DetailsState {
                        fic: &fic,
                        all_shelves: &assignable_shelves,
                        selection_shelf_ids: &self.cache.selection_shelf_ids,
                    },
                );
            });

        self.dispatch_details_outcome(id, outcome);
    }

    pub(super) fn paint_selection_bar(&mut self, host: &mut egui::Ui) {
        let selection_ids = self.selection.ids_vec();
        if selection_ids.is_empty() || !self.current_view.shows_library() {
            return;
        }
        let assignable_shelves = self.assignable_shelves();
        let mut outcome = selection_bar::Outcome::None;
        egui::Panel::bottom("ficflow-selection-bar")
            .resizable(false)
            .show(host, |ui| {
                outcome = selection_bar::draw(
                    ui,
                    SelectionBarState {
                        selection_ids: &selection_ids,
                        current_view: &self.current_view,
                        all_shelves: &assignable_shelves,
                    },
                );
            });
        match outcome {
            selection_bar::Outcome::None => {}
            selection_bar::Outcome::SetStatus(status) => {
                let (succeeded, failed) = self.bulk_set_status(&selection_ids, status);
                self.toast_bulk_result("Status updated", succeeded, failed);
            }
            selection_bar::Outcome::AddToShelf(shelf_id) => {
                let (succeeded, failed) = self.bulk_add_to_shelf(&selection_ids, shelf_id);
                self.toast_bulk_result("Added to shelf", succeeded, failed);
            }
            selection_bar::Outcome::RemoveFromShelf(shelf_id) => {
                let (succeeded, failed) = self.bulk_remove_from_shelf(&selection_ids, shelf_id);
                self.toast_bulk_result("Removed from shelf", succeeded, failed);
            }
            selection_bar::Outcome::RequestDelete => {
                self.active_modal = ActiveModal::DeleteFics(selection_ids);
            }
            selection_bar::Outcome::ClearSelection => {
                self.clear_selection();
            }
        }
    }

    pub(super) fn paint_central(&mut self, host: &mut egui::Ui) {
        let mut table_outcome = TableOutcome::default();
        let mut empty_area_clicked = false;
        let prev_selection = self.selection.current().clone();
        let view_title = self.current_view.header_title(&self.cache.shelves);
        egui::CentralPanel::default().show(host, |ui| {
            self.draw_central_header(ui, &view_title);
            ui.add_space(6.0);
            match self.current_view {
                View::AllFics | View::ByStatus(_) | View::Shelf(_) => {
                    table_outcome = library_view::draw(
                        ui,
                        LibraryViewState {
                            fics: &self.cache.fics,
                            generation: self.cache.generation,
                            sort: &mut self.sort,
                            search_query: &self.search_query,
                            visible_columns: &self.config.visible_columns,
                            selection: &mut self.selection,
                            view: &self.current_view,
                            shelf_members: &self.cache.shelf_members,
                        },
                    );
                }
                View::Tasks => tasks_view::draw(
                    ui,
                    TasksViewState {
                        executor: &self.task_executor,
                        filter: &mut self.task_filter,
                    },
                ),
                View::Settings => {
                    self.update_check.start_once(ui.ctx());
                    let outcome = settings_view::draw(
                        ui,
                        &mut self.config,
                        &self.current_db_path,
                        self.update_check.newer_release(),
                    );
                    if outcome.config_changed {
                        self.save_config();
                    }
                    // The native picker is opened later from `ui()`, which holds
                    // the window handle needed to parent the dialog above us.
                    if outcome.request.is_some() {
                        self.pending_library_request = outcome.request;
                    }
                }
            }
            // Empty-area click-sink. Table rows have their own click
            // sense, so the panel's outer response never fires below
            // the last row — allocate the leftover space here.
            let remaining = ui.available_size();
            if remaining.x > 0.0 && remaining.y > 0.0 {
                let resp = ui.allocate_response(remaining, egui::Sense::click());
                if resp.clicked() {
                    empty_area_clicked = true;
                }
            }
        });
        if empty_area_clicked && self.current_view.shows_library() {
            self.clear_selection();
        }
        if *self.selection.current() != prev_selection {
            self.refresh_selection_shelf_ids();
        }
        // Selected fic got deleted this frame: drop the orphan selection.
        if let Selection::Single(id) = *self.selection.current()
            && self.cache.fic(id).is_none()
        {
            self.clear_selection();
        }
        if table_outcome.sort_changed {
            self.config.default_sort = self.sort;
            self.save_config();
        }
        if let Some((col, target, after)) = table_outcome.column_reorder {
            self.config.reorder_visible_column(col, target, after);
            self.save_config();
        }
        if let Some(col) = table_outcome.column_removed {
            self.config.visible_columns.retain(|c| *c != col);
            self.save_config();
        }
    }
    pub(super) fn drain_worker_events(&mut self, ctx: &egui::Context) {
        let completions = self.task_executor.take_completions();
        if !completions.is_empty() {
            for title in &completions {
                self.toasts
                    .success(format!("Added \u{201C}{}\u{201D}", title));
            }
            self.cache.reload_fics(&self.connection);
            self.refresh_shelf_members();
            self.refresh_selection_shelf_ids();
        }
        let refreshes = self.task_executor.take_refreshes();
        if !refreshes.is_empty() {
            self.toasts
                .success(format!("Refreshed {} fanfiction(s)", refreshes.len()));
            self.cache.reload_fics(&self.connection);
            self.refresh_shelf_members();
        }
        if self.task_executor.has_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

impl FicflowApp {
    /// Search bar is overlaid via an `Area` anchored to the window's
    /// horizontal centre — so resizing the sidebar / details panel
    /// doesn't slide it around — but clamped to the central panel's
    /// rect so it never paints over the right panel.
    pub(super) fn draw_central_header(&mut self, ui: &mut egui::Ui, view_title: &str) {
        // Header buttons collapse to icons (`+`, `≡`) below this
        // width to leave room for the search-bar overlay.
        let compact = ui.available_width() < 520.0;
        // `ui.horizontal(...)` claims only its children's width; we
        // need the panel's full rect to clamp the search overlay.
        let panel_rect = ui.available_rect_before_wrap();
        let row_resp = ui.horizontal(|ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(view_title)
                        .family(egui::FontFamily::Name(theme::NEUE_FAMILY.into()))
                        .size(20.0)
                        .color(theme::accent(ui.visuals())),
                )
                .selectable(false),
            );
            if !self.current_view.shows_library() {
                return;
            }
            let visible = library_view::visible_count(
                ui.ctx(),
                &self.cache.fics,
                self.cache.generation,
                &self.current_view,
                &self.cache.shelf_members,
                &self.search_query,
                self.sort,
            );
            let suffix = if visible == 1 { "fic" } else { "fics" };
            ui.label(egui::RichText::new(format!("{} {}", visible, suffix)).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let cols_label = if compact {
                    "\u{2261}"
                } else {
                    "Manage Columns"
                };
                if ui
                    .button(cols_label)
                    .on_hover_text("Manage Columns")
                    .clicked()
                {
                    self.show_column_picker = !self.show_column_picker;
                }
                let add_label = if compact { "+" } else { "+ Add Fic" };
                if ui.button(add_label).on_hover_text("Add Fic").clicked() {
                    self.active_modal = ActiveModal::AddFic(AddFicState::default());
                }
            });
        });

        if self.current_view.shows_library() {
            const SEARCH_W_MAX: f32 = 300.0;
            const SEARCH_W_MIN: f32 = 120.0;
            const SEARCH_H: f32 = 22.0;
            const EDGE_GAP: f32 = 8.0;
            // Approximate widths the row reserves for title+count on
            // the left and action buttons on the right; the search
            // bar fills whatever's left.
            let buttons_reserve = if compact { 90.0 } else { 220.0 };
            let title_reserve = 200.0;
            let avail_for_search =
                panel_rect.width() - buttons_reserve - title_reserve - 2.0 * EDGE_GAP;
            if avail_for_search >= SEARCH_W_MIN {
                let search_w = avail_for_search.min(SEARCH_W_MAX);
                let screen = ui.ctx().content_rect();
                let row_rect = row_resp.response.rect;
                let desired_x = screen.center().x - search_w / 2.0;
                let max_x = panel_rect.right() - buttons_reserve - EDGE_GAP - search_w;
                let min_x = panel_rect.left() + title_reserve + EDGE_GAP;
                let clamped_x = desired_x.clamp(min_x, max_x);
                let pos = egui::pos2(clamped_x, row_rect.center().y - SEARCH_H / 2.0);
                egui::Area::new(egui::Id::unique("ficflow-search-overlay"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(pos)
                    .show(ui.ctx(), |area_ui| {
                        self.draw_search_field(area_ui, search_w);
                    });
            }
        }
    }

    /// Magnifying-glass glyph + borderless TextEdit inside a Frame so
    /// the icon sits *inside* the apparent input boundary (a hint
    /// text would disappear once the user starts typing).
    pub(super) fn draw_search_field(&mut self, ui: &mut egui::Ui, width: f32) {
        let stroke = ui.visuals().widgets.inactive.bg_stroke;
        let fill = ui.visuals().extreme_bg_color;
        let weak = ui.visuals().weak_text_color();

        ui.allocate_ui(egui::vec2(width, 22.0), |ui| {
            egui::Frame::default()
                .fill(fill)
                .stroke(stroke)
                .corner_radius(2)
                .inner_margin(egui::Margin::symmetric(6, 2))
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(egui::RichText::new("\u{1F50D}").color(weak));
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text("Search title, author, fandom, tags…")
                                .frame(egui::Frame::NONE)
                                .desired_width(f32::INFINITY),
                        );
                        if self.focus_search_pending {
                            resp.request_focus();
                            self.focus_search_pending = false;
                        }
                    });
                });
        });
    }
}

impl FicflowApp {
    /// egui's built-in dnd doesn't ship a drag-preview, so we paint a
    /// label near the cursor whenever there's an active payload.
    pub(super) fn draw_drag_preview(&self, ctx: &egui::Context) {
        let label = if let Some(payload) = egui::DragAndDrop::payload::<Vec<u64>>(ctx) {
            match payload.as_slice() {
                [single] => self
                    .cache
                    .fic(*single)
                    .map(|f| f.title.clone())
                    .unwrap_or_else(|| "(unknown)".to_string()),
                ids => format!("{} fanfictions", ids.len()),
            }
        } else if let Some(column) = egui::DragAndDrop::payload::<ColumnKey>(ctx) {
            column.label().to_string()
        } else if let Some(dragged) = egui::DragAndDrop::payload::<sidebar::ShelfDrag>(ctx) {
            self.cache
                .shelf(dragged.0)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "(unknown)".to_string())
        } else {
            return;
        };
        let Some(pointer) = ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        egui::Area::new(egui::Id::unique("ficflow-drag-preview"))
            .fixed_pos(pointer + egui::Vec2::new(14.0, 14.0))
            .order(egui::Order::Tooltip)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(260.0);
                    ui.add(egui::Label::new(label).truncate());
                });
            });
    }
}
