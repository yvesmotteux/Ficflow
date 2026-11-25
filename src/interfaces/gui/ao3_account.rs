use std::sync::{Arc, OnceLock};
use std::thread;

use crate::infrastructure::external::ao3::session::{SessionStore, log_in};

type LoginSlot = Arc<OnceLock<Result<String, String>>>;

pub struct Ao3Account {
    store: Arc<dyn SessionStore>,
    site: String,
    username: Option<String>,
    pending: Option<LoginSlot>,
    error: Option<String>,
    pub username_input: String,
    pub password_input: String,
}

impl Ao3Account {
    pub fn new(store: Arc<dyn SessionStore>, site: String) -> Self {
        let username = store.load().map(|session| session.username);
        Self {
            store,
            site,
            username,
            pending: None,
            error: None,
            username_input: String::new(),
            password_input: String::new(),
        }
    }

    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn logging_in(&self) -> bool {
        self.pending.is_some()
    }

    pub fn reload(&mut self) {
        if !self.logging_in() {
            self.username = self.store.load().map(|session| session.username);
        }
    }

    pub fn log_in(&mut self, ctx: &egui::Context) {
        if self.logging_in() {
            return;
        }
        let username = self.username_input.trim().to_string();
        let password = std::mem::take(&mut self.password_input);
        if username.is_empty() || password.is_empty() {
            self.error = Some("Enter your AO3 user name and password.".to_string());
            return;
        }
        self.error = None;
        let slot: LoginSlot = Arc::new(OnceLock::new());
        self.pending = Some(Arc::clone(&slot));
        let store = Arc::clone(&self.store);
        let site = self.site.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let outcome = log_in(&site, &username, &password)
                .and_then(|session| store.save(&session).map(|()| session.username))
                .map_err(|err| err.to_string());
            let _ = slot.set(outcome);
            ctx.request_repaint();
        });
    }

    pub fn poll(&mut self) {
        let Some(outcome) = self.pending.as_ref().and_then(|slot| slot.get()) else {
            return;
        };
        match outcome {
            Ok(username) => {
                self.username = Some(username.clone());
                self.username_input.clear();
            }
            Err(message) => self.error = Some(message.clone()),
        }
        self.pending = None;
    }

    pub fn log_out(&mut self) {
        match self.store.clear() {
            Ok(()) => {
                self.username = None;
                self.error = None;
            }
            Err(err) => self.error = Some(err.to_string()),
        }
    }
}
