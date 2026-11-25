use chrono::Utc;
use egui::{Align, Layout, RichText, ScrollArea, Ui};

use crate::domain::inbox::{InboxMessage, MessageContent};

use super::super::format::{ao3_work_url, relative_time};

pub enum Outcome {
    None,
    Open { message_id: u64, url: String },
    ShowInLibrary { message_id: u64, fic_id: u64 },
    Delete(u64),
    MarkAllRead,
    ClearAll,
}

pub fn draw(ui: &mut Ui, messages: &[InboxMessage], in_library: impl Fn(u64) -> bool) -> Outcome {
    let mut outcome = Outcome::None;
    let unread = messages.iter().filter(|m| m.is_unread()).count();

    ui.horizontal(|ui| {
        ui.label(match unread {
            0 => "No unread messages".to_string(),
            1 => "1 unread message".to_string(),
            n => format!("{n} unread messages"),
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add_enabled(!messages.is_empty(), egui::Button::new("Clear all"))
                .clicked()
            {
                outcome = Outcome::ClearAll;
            }
            if ui
                .add_enabled(unread > 0, egui::Button::new("Mark all as read"))
                .clicked()
            {
                outcome = Outcome::MarkAllRead;
            }
        });
    });
    ui.separator();

    if messages.is_empty() {
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Nothing here yet. New chapters of the fics in your library will show up here.",
            )
            .italics()
            .weak(),
        );
        return outcome;
    }

    let now = Utc::now();
    ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for message in messages {
                if let Some(action) = message_row(ui, message, now, &in_library) {
                    outcome = action;
                }
                ui.separator();
            }
        });
    outcome
}

fn message_row(
    ui: &mut Ui,
    message: &InboxMessage,
    now: chrono::DateTime<Utc>,
    in_library: &impl Fn(u64) -> bool,
) -> Option<Outcome> {
    let MessageContent::NewChapters {
        fic_id,
        title,
        authors,
        fandoms,
        previous_count,
        new_count,
        first_new_chapter_id,
    } = &message.content;
    let mut outcome = None;
    let unread_color = ui.visuals().selection.bg_fill;

    ui.horizontal(|ui| {
        let dot = if message.is_unread() { "\u{25CF}" } else { " " };
        ui.label(RichText::new(dot).color(unread_color));
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(title).strong());
                ui.label(format!("by {}", authors.join(", ")));
            });
            let added = new_count.saturating_sub(*previous_count);
            let summary = if added == 1 {
                format!("New chapter {new_count} is out")
            } else {
                format!(
                    "{added} new chapters are out ({}\u{2013}{new_count})",
                    previous_count + 1
                )
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(summary);
                ui.label(RichText::new(relative_time(message.created_at, now)).weak());
            });
            ui.label(RichText::new(fandoms.join(", ")).weak().small());
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .small_button("\u{2715}")
                .on_hover_text("Delete message")
                .clicked()
            {
                outcome = Some(Outcome::Delete(message.id));
            }
            if in_library(*fic_id) && ui.button("Show in library").clicked() {
                outcome = Some(Outcome::ShowInLibrary {
                    message_id: message.id,
                    fic_id: *fic_id,
                });
            }
            if ui.button("Read on AO3 \u{2197}").clicked() {
                outcome = Some(Outcome::Open {
                    message_id: message.id,
                    url: ao3_work_url(*fic_id, *first_new_chapter_id),
                });
            }
        });
    });
    outcome
}
