use egui::Ui;

use crate::domain::fanfiction::UserRating;

/// Renders a 5-star rating widget. Clicking star N sets the rating to N;
/// clicking the currently-selected star clears the rating. Returns true if
/// the rating value changed this frame so the caller can persist it.
///
/// Uses unicode ★ / ☆ — egui's bundled emoji font covers these glyphs even
/// without our own font installed.
pub fn star_rating(ui: &mut Ui, rating: &mut Option<UserRating>) -> bool {
    let mut changed = false;
    let current = rating.map(rating_to_u8).unwrap_or(0);
    ui.horizontal(|ui| {
        for star in 1..=5u8 {
            let label = if star <= current {
                "\u{2605}"
            } else {
                "\u{2606}"
            };
            if ui.small_button(label).clicked() {
                changed = true;
                *rating = if Some(star) == rating.map(rating_to_u8) {
                    None
                } else {
                    Some(rating_from_u8(star))
                };
            }
        }
        if rating.is_some() && ui.small_button("clear").clicked() {
            *rating = None;
            changed = true;
        }
    });
    changed
}

fn rating_to_u8(rating: UserRating) -> u8 {
    rating as u8
}

fn rating_from_u8(n: u8) -> UserRating {
    match n {
        1 => UserRating::One,
        2 => UserRating::Two,
        3 => UserRating::Three,
        4 => UserRating::Four,
        _ => UserRating::Five,
    }
}
