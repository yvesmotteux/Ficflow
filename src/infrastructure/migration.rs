use rusqlite::{Connection, Result};
use rusqlite_migration::{Migrations, M};
use std::error::Error;

pub fn run_migrations(conn: &mut Connection) -> Result<(), Box<dyn Error>> {
    let migrations = Migrations::new(vec![
        M::up(r#"
            CREATE TABLE IF NOT EXISTS fanfiction (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                authors TEXT NOT NULL,
                categories TEXT,
                chapters_total INTEGER,
                chapters_published INTEGER NOT NULL,
                characters TEXT,
                complete BOOLEAN NOT NULL,
                fandoms TEXT NOT NULL,
                hits INTEGER NOT NULL,
                kudos INTEGER NOT NULL,
                language TEXT NOT NULL,
                rating TEXT NOT NULL,
                relationships TEXT,
                restricted BOOLEAN NOT NULL,
                summary TEXT NOT NULL,
                tags TEXT,
                warnings TEXT NOT NULL,
                words INTEGER NOT NULL,
                date_published TEXT NOT NULL,
                date_updated TEXT NOT NULL,
                last_chapter_read INTEGER,
                reading_status TEXT NOT NULL,
                read_count INTEGER NOT NULL,
                user_rating INTEGER,
                personal_note TEXT,
                last_checked_date TEXT NOT NULL
            )
        "#),
    ]);

    migrations.to_latest(conn)?;

    Ok(())
}
