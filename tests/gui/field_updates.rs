//! Group B — personal-field updates.
//!
//! Covers `update_status`, `update_chapters`, `update_read_count`,
//! `update_rating`, `update_note`. Each scenario mutates one field via
//! the FicflowApp control surface (the same code path the Your Info
//! widgets dispatch through), then asserts both the in-memory cache
//! and the underlying SQLite row reflect the change.

#[cfg(test)]
mod tests {
    use ficflow::domain::fanfiction::{FanfictionOps, ReadingStatus, UserRating};
    use ficflow::infrastructure::SqliteRepository;

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    /// Convenience: build a harness with one seeded fic of the given id.
    /// Returns the fic id so tests don't have to repeat the literal.
    fn given_harness_with_one_fic(id: u64) -> (GuiHarness, u64) {
        let (conn, db_path, td) = fixtures::given_test_database();
        let fic = fixtures::given_sample_fanfiction(id, "T");
        fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        let h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        (h, id)
    }

    fn db_fic(h: &GuiHarness, id: u64) -> ficflow::domain::fanfiction::Fanfiction {
        SqliteRepository::new(&h.conn)
            .get_fanfiction_by_id(id)
            .unwrap()
    }

    /// B7 — change the Status combo: in-memory cache, DB row, and the
    /// library_counts that the sidebar reads from all reflect the new
    /// status on the next frame.
    #[test]
    fn changing_status_persists_and_shifts_counts() {
        let (mut h, id) = given_harness_with_one_fic(101);
        h.step_n(1);
        // Sample fic ships as `PlanToRead`.
        assert_eq!(h.app.fics()[0].reading_status, ReadingStatus::PlanToRead);

        h.app.set_status(id, ReadingStatus::Read).unwrap();
        h.step();

        assert_eq!(h.app.fics()[0].reading_status, ReadingStatus::Read);
        assert_eq!(db_fic(&h, id).reading_status, ReadingStatus::Read);

        // The sidebar count for `PlanToRead` is just the in-memory
        // status reduce — verify by counting directly off `fics()`.
        let plan_count = h
            .app
            .fics()
            .iter()
            .filter(|f| f.reading_status == ReadingStatus::PlanToRead)
            .count();
        let read_count = h
            .app
            .fics()
            .iter()
            .filter(|f| f.reading_status == ReadingStatus::Read)
            .count();
        assert_eq!(plan_count, 0);
        assert_eq!(read_count, 1);
    }

    /// B8 — bumping the chapter marker drives both the chapter field
    /// itself and the status auto-promotion that `update_chapters`
    /// performs (PlanToRead/Paused → InProgress on partial; → Read on
    /// final, with read_count incrementing). Clamps to `chapters_total`.
    #[test]
    fn last_chapter_marker_clamps_and_drives_status() {
        let (mut h, id) = given_harness_with_one_fic(102);
        h.step_n(1);
        // Sample: chapters_total=Some(2), chapters_published=1,
        // last_chapter_read=None, reading_status=PlanToRead.

        // Mid-fic chapter (chapter 1 of 2). Should auto-bump status.
        h.app.set_last_chapter(id, 1).unwrap();
        let mid = &h.app.fics()[0];
        assert_eq!(mid.last_chapter_read, Some(1));
        assert_eq!(mid.reading_status, ReadingStatus::InProgress);
        assert_eq!(mid.read_count, 0, "non-final read shouldn't bump count");

        // Try to set chapter past total: clamped to 2 (chapters_total),
        // and because that's the final chapter, status → Read and
        // read_count increments.
        h.app.set_last_chapter(id, 5).unwrap();
        let final_ = &h.app.fics()[0];
        assert_eq!(final_.last_chapter_read, Some(2), "clamp at chapters_total");
        assert_eq!(final_.reading_status, ReadingStatus::Read);
        assert_eq!(final_.read_count, 1);

        // DB agrees.
        let row = db_fic(&h, id);
        assert_eq!(row.last_chapter_read, Some(2));
        assert_eq!(row.reading_status, ReadingStatus::Read);
    }

    /// B9 — set the read counter directly. Independent of the chapter
    /// auto-promotion path; just a counter the user can edit.
    #[test]
    fn read_count_persists() {
        let (mut h, id) = given_harness_with_one_fic(103);
        h.step_n(1);
        assert_eq!(h.app.fics()[0].read_count, 0);

        h.app.set_read_count(id, 7).unwrap();

        assert_eq!(h.app.fics()[0].read_count, 7);
        assert_eq!(db_fic(&h, id).read_count, 7);
    }

    /// B10 — star rating: setting a value persists, passing `None`
    /// clears it back to "no rating".
    #[test]
    fn user_rating_round_trips_and_can_be_cleared() {
        let (mut h, id) = given_harness_with_one_fic(104);
        h.step_n(1);
        assert!(h.app.fics()[0].user_rating.is_none());

        h.app.set_user_rating(id, Some(UserRating::Four)).unwrap();
        assert_eq!(h.app.fics()[0].user_rating, Some(UserRating::Four));
        assert_eq!(db_fic(&h, id).user_rating, Some(UserRating::Four));

        h.app.set_user_rating(id, None).unwrap();
        assert!(h.app.fics()[0].user_rating.is_none());
        assert!(db_fic(&h, id).user_rating.is_none());
    }

    /// B11 — typing a note and committing it (TextEdit `lost_focus`
    /// in the GUI) writes through to the DB. Survives a "relaunch" —
    /// modelled as a fresh repo read against the same SQLite file.
    #[test]
    fn note_persists_through_to_db() {
        let (mut h, id) = given_harness_with_one_fic(105);
        h.step_n(1);
        assert!(h.app.fics()[0].personal_note.is_none());

        h.app.set_note(id, Some("Re-read every Christmas")).unwrap();

        assert_eq!(
            h.app.fics()[0].personal_note.as_deref(),
            Some("Re-read every Christmas")
        );
        // Same row read straight from SQLite — no in-memory caching.
        assert_eq!(
            db_fic(&h, id).personal_note.as_deref(),
            Some("Re-read every Christmas")
        );
    }

    /// B12 — erasing the note (passing `None`, equivalent to clearing
    /// the textarea and tabbing out) removes the row's personal_note,
    /// not just blanks it to an empty string.
    #[test]
    fn clearing_note_writes_none_not_empty_string() {
        let (mut h, id) = given_harness_with_one_fic(106);
        h.step_n(1);
        h.app.set_note(id, Some("temp")).unwrap();
        assert_eq!(h.app.fics()[0].personal_note.as_deref(), Some("temp"));

        h.app.set_note(id, None).unwrap();

        assert!(h.app.fics()[0].personal_note.is_none());
        assert!(
            db_fic(&h, id).personal_note.is_none(),
            "DB should store NULL, not empty string"
        );
    }
}
