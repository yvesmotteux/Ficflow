use std::thread;

use notify_rust::Notification;

use crate::domain::inbox::{InboxMessage, MessageContent};

const MAX_LISTED_TITLES: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct DesktopNotification {
    pub summary: String,
    pub body: String,
}

pub type ClickHandler = Box<dyn FnOnce(Option<String>) + Send>;

pub trait DesktopNotifier: Send + Sync {
    fn notify(&self, notification: DesktopNotification, on_click: ClickHandler);
}

pub struct SystemNotifier;

impl DesktopNotifier for SystemNotifier {
    fn notify(&self, notification: DesktopNotification, on_click: ClickHandler) {
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
fn show(mut notification: Notification, on_click: ClickHandler) {
    notification.action("default", "Open inbox");
    let signals = notification_signals();
    let handle = match notification.show() {
        Ok(handle) => handle,
        Err(err) => {
            log::warn!("couldn't show desktop notification: {err}");
            return;
        }
    };
    match signals {
        Ok(signals) => {
            if let Some(token) = wait_for_click(signals, handle.id()) {
                on_click(token);
            }
        }
        Err(err) => {
            log::warn!("couldn't listen for notification clicks: {err}");
            handle.wait_for_action(|action| {
                if action == "default" {
                    on_click(None);
                }
            });
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn notification_signals() -> zbus::Result<zbus::blocking::MessageIterator> {
    let connection = zbus::blocking::Connection::session()?;
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.Notifications")?
        .path("/org/freedesktop/Notifications")?
        .build();
    zbus::blocking::MessageIterator::for_match_rule(rule, &connection, None)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn wait_for_click(signals: zbus::blocking::MessageIterator, id: u32) -> Option<Option<String>> {
    let mut token = None;
    for message in signals {
        let message = message.ok()?;
        let header = message.header();
        let body = message.body();
        match header.member().map(|m| m.as_str()) {
            Some("ActivationToken") => {
                if let Ok((sent_for, sent_token)) = body.deserialize::<(u32, String)>()
                    && sent_for == id
                {
                    token = Some(sent_token);
                }
            }
            Some("ActionInvoked") => {
                if let Ok((sent_for, action)) = body.deserialize::<(u32, String)>()
                    && sent_for == id
                    && action == "default"
                {
                    return Some(token);
                }
            }
            Some("NotificationClosed") => {
                if let Ok((sent_for, _)) = body.deserialize::<(u32, u32)>()
                    && sent_for == id
                {
                    return None;
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn show(notification: Notification, _on_click: ClickHandler) {
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
