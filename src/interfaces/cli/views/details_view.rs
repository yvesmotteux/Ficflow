use std::fmt::{Display, Write};

use crate::domain::fanfiction::Fanfiction;

pub fn render_fanfiction_details(fic: &Fanfiction) -> String {
    let mut out = String::new();
    let mut row = |label: &str, value: &dyn Display| {
        let _ = writeln!(out, "{:<21}{}", format!("{}:", label), value);
    };
    let date_format = "%Y-%m-%d %H:%M:%S UTC";
    let yes_no = |b: bool| if b { "Yes" } else { "No" };

    row("ID", &fic.id);
    row("Title", &fic.title);
    row("Author(s)", &fic.authors.join(", "));
    row("Fandom(s)", &fic.fandoms.join(", "));
    row("Language", &fic.language);
    row("Rating", &fic.rating);
    row("Words", &fic.words);
    row(
        "Chapters",
        &format!(
            "{}/{}",
            fic.chapters_published,
            fic.chapters_total
                .map_or("?".to_string(), |c| c.to_string())
        ),
    );
    row("Complete", &yes_no(fic.complete));
    row("Restricted", &yes_no(fic.restricted));
    row("Hits", &fic.hits);
    row("Kudos", &fic.kudos);
    row("Published", &fic.date_published.format(date_format));
    row("Updated", &fic.date_updated.format(date_format));
    row("Last Checked", &fic.last_checked_date.format(date_format));
    if let Some(categories) = &fic.categories {
        row("Categories", &join(categories));
    }
    if let Some(characters) = &fic.characters {
        row("Characters", &characters.join(", "));
    }
    if let Some(relationships) = &fic.relationships {
        row("Relationships", &relationships.join(", "));
    }
    if let Some(tags) = &fic.tags {
        row("Tags", &tags.join(", "));
    }
    if !fic.warnings.is_empty() {
        row("Warnings", &join(&fic.warnings));
    }
    row("Reading Status", &fic.reading_status);
    row("Read Count", &fic.read_count);
    if let Some(last_chapter) = fic.last_chapter_read {
        row("Last Chapter Read", &last_chapter);
    }
    if let Some(rating) = fic.user_rating {
        row("Your Rating", &format!("{} / 5", rating as u8));
    }

    if let Some(note) = &fic.personal_note {
        let _ = writeln!(out, "\nPersonal Note:\n{}", note);
    }
    let _ = write!(out, "\nSummary:\n{}", fic.summary);
    out
}

fn join<T: Display>(items: &[T]) -> String {
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
