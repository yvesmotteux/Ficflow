use std::sync::{Arc, OnceLock};
use std::thread;

use crate::infrastructure::external::github::{Release, fetch_latest_release};
use crate::version::VERSION;

pub struct UpdateCheck {
    url: String,
    latest: Option<Arc<OnceLock<Option<Release>>>>,
}

impl UpdateCheck {
    pub fn new(url: String) -> Self {
        Self { url, latest: None }
    }

    pub fn start_once(&mut self, ctx: &egui::Context) {
        if self.latest.is_some() {
            return;
        }
        let slot = Arc::new(OnceLock::new());
        self.latest = Some(Arc::clone(&slot));
        let url = self.url.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let release = fetch_latest_release(&url)
                .inspect_err(|err| log::warn!("couldn't check for a newer Ficflow release: {err}"))
                .ok();
            let _ = slot.set(release);
            ctx.request_repaint();
        });
    }

    pub fn newer_release(&self) -> Option<&Release> {
        self.latest
            .as_ref()?
            .get()?
            .as_ref()
            .filter(|release| release.is_newer_than(VERSION))
    }
}
