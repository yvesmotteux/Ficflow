use super::*;

impl FicflowApp {
    /// eframe's window persistence drops `maximized` — track it
    /// ourselves: re-apply on first frame, then persist on toggle.
    pub(super) fn sync_window_state(&mut self, ctx: &egui::Context) {
        if !self.initial_window_state_applied {
            if self.config.window_fullscreen {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            } else if self.config.window_maximized {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            }
            self.initial_window_state_applied = true;
            return;
        }

        let (maximized, fullscreen) = ctx.input(|i| {
            (
                i.viewport().maximized.unwrap_or(false),
                i.viewport().fullscreen.unwrap_or(false),
            )
        });
        if maximized != self.config.window_maximized || fullscreen != self.config.window_fullscreen
        {
            self.config.window_maximized = maximized;
            self.config.window_fullscreen = fullscreen;
            self.save_config();
        }
    }

    /// Reconciles zoom changes from egui's built-in Ctrl/Cmd +/-/0 shortcut,
    /// which mutates `zoom_factor` directly (bypassing our code) and uses
    /// its own 0.2..=5.0 range, not `TEXT_ZOOM_RANGE`.
    pub(super) fn sync_text_zoom(&mut self, ctx: &egui::Context) {
        let raw = ctx.zoom_factor();
        let clamped = config::clamp_zoom(raw);
        if (raw - clamped).abs() > f32::EPSILON {
            ctx.set_zoom_factor(clamped);
        }
        if (clamped - self.config.text_zoom).abs() > f32::EPSILON {
            self.config.text_zoom = clamped;
            self.save_config();
        }
    }

    pub(super) fn handle_close_request(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quit_confirmed
            && self.task_executor.has_running()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.active_modal = ActiveModal::ConfirmQuit;
        }
    }
}

impl eframe::App for FicflowApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.render(ui);
        if let Some(request) = self.pending_library_request.take() {
            self.open_library_picker(request, frame);
        }
        if let Some(token) = self.pending_activation_token.take() {
            raise_with_token(frame, &token);
        }
    }

    /// Transparent so the chrome's painted edges show through the
    /// borderless undecorated window (see NativeOptions in `gui/mod.rs`).
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn on_exit(&mut self) {
        self.checkpoint_wal();
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn raise_with_token(frame: &eframe::Frame, token: &str) {
    use super::super::window_activation;
    if window_activation::is_wayland(frame)
        && let Err(err) = window_activation::activate(frame, token)
    {
        log::warn!("couldn't bring the window to the front: {err}");
    }
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn raise_with_token(_frame: &eframe::Frame, _token: &str) {}
