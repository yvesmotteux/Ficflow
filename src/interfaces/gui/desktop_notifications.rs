use std::thread;

use notify_rust::Notification;

use crate::domain::inbox::{InboxMessage, MessageContent};

const MAX_LISTED_TITLES: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct DesktopNotification {
    pub summary: String,
    pub body: String,
}

pub trait DesktopNotifier: Send + Sync {
    fn notify(&self, notification: DesktopNotification, on_click: Box<dyn FnOnce() + Send>);
}

pub struct SystemNotifier;

impl DesktopNotifier for SystemNotifier {
    fn notify(&self, notification: DesktopNotification, on_click: Box<dyn FnOnce() + Send>) {
        thread::spawn(move || {
            let mut system = Notification::new();
            system
                .appname("Ficflow")
                .summary(&notification.summary)
                .body(&notification.body);
            show(system, on_click);
        });
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn show(mut notification: Notification, on_click: Box<dyn FnOnce() + Send>) {
    notification.action("default", "Open inbox");
    match notification.show() {
        Ok(handle) => handle.wait_for_action(|action| {
            if action == "default" {
                on_click();
            }
        }),
        Err(err) => log::warn!("couldn't show desktop notification: {err}"),
    }
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn show(notification: Notification, _on_click: Box<dyn FnOnce() + Send>) {
    if let Err(err) = notification.show() {
        log::warn!("couldn't show desktop notification: {err}");
    }
}

pub fn new_chapters_notification(messages: &[InboxMessage]) -> Option<DesktopNotification> {
    let fics: Vec<(&str, &[String], u32)> = messages
        .iter()
        .map(|message| match &message.content {
            MessageContent::NewChapters {
                title,
                authors,
                previous_count,
                new_count,
                ..
            } => (
                title.as_str(),
                authors.as_slice(),
                new_count.saturating_sub(*previous_count),
            ),
        })
        .collect();
    match fics.as_slice() {
        [] => None,
        [(title, authors, added)] => Some(DesktopNotification {
            summary: if *added == 1 {
                format!("New chapter of {title}")
            } else {
                format!("{added} new chapters of {title}")
            },
            body: format!("by {}", authors.join(", ")),
        }),
        many => {
            let mut titles: Vec<&str> = many
                .iter()
                .take(MAX_LISTED_TITLES)
                .map(|(title, ..)| *title)
                .collect();
            if many.len() > MAX_LISTED_TITLES {
                titles.push("…");
            }
            Some(DesktopNotification {
                summary: format!("New chapters for {} fics", many.len()),
                body: titles.join("\n"),
            })
        }
    }
}
