use rusqlite::{Connection, OpenFlags};
use std::error::Error;
use tempfile::TempDir;

use crate::common::fixtures;

#[cfg(test)]
mod tests {
    use ficflow::domain::fanfiction::FanfictionOps;
    use ficflow::domain::shelf::ShelfOps;
    use ficflow::error::FicflowError;
    use ficflow::infrastructure::persistence::database::migration::run_migrations;
    use ficflow::infrastructure::persistence::repository::SqliteRepository;

    use super::*;

    fn setup_test_db() -> (Connection, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        let mut conn = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .expect("Failed to open database with write permissions");
        run_migrations(&mut conn).expect("Failed to run migrations");
        // FK enforcement matches production; without this PRAGMA, cascades
        // and FK rejections we rely on (e.g. fic_shelf row deletion when a
        // fic is hard-removed) wouldn't fire.
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .expect("Failed to enable foreign keys");
        (conn, temp_dir)
    }

    #[test]
    fn test_create_shelf_persists_with_trimmed_name() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let shelf = repo.create_shelf("  Favorites  ", None)?;

        assert_eq!(
            shelf.name, "Favorites",
            "leading/trailing whitespace trimmed"
        );
        let listed = repo.list_shelves()?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, shelf.id);
        Ok(())
    }

    #[test]
    fn test_create_shelf_rejects_empty_name() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.create_shelf("", None).unwrap_err();
        assert!(
            matches!(err, FicflowError::InvalidInput(_)),
            "expected InvalidInput, got {:?}",
            err
        );
    }

    #[test]
    fn test_create_shelf_rejects_whitespace_only_name() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.create_shelf("   \t  ", None).unwrap_err();
        assert!(matches!(err, FicflowError::InvalidInput(_)));
    }

    #[test]
    fn test_get_shelf_by_id_returns_shelf() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let created = repo.create_shelf("Long Reads", None)?;
        let fetched = repo.get_shelf_by_id(created.id)?;

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "Long Reads");
        Ok(())
    }

    #[test]
    fn test_get_shelf_by_id_returns_not_found_for_missing() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.get_shelf_by_id(9999).unwrap_err();
        assert!(matches!(
            err,
            FicflowError::ShelfNotFound { shelf_id: 9999 }
        ));
    }

    #[test]
    fn test_list_shelves_sorts_by_name_case_insensitive() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        repo.create_shelf("zebra", None)?;
        repo.create_shelf("Apple", None)?;
        repo.create_shelf("Mango", None)?;

        let listed: Vec<String> = repo.list_shelves()?.into_iter().map(|s| s.name).collect();
        assert_eq!(listed, vec!["Apple", "Mango", "zebra"]);
        Ok(())
    }

    #[test]
    fn test_list_shelves_orders_pinned_before_unpinned_regardless_of_name()
    -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        repo.create_shelf("Apple", None)?;
        let zebra = repo.create_shelf("zebra", None)?;
        repo.create_shelf("Mango", None)?;
        repo.set_shelf_pinned(zebra.id, true)?;

        let listed: Vec<String> = repo.list_shelves()?.into_iter().map(|s| s.name).collect();
        assert_eq!(listed, vec!["zebra", "Apple", "Mango"]);
        Ok(())
    }

    #[test]
    fn test_set_shelf_pinned_toggles_and_rejects_missing_shelf() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let shelf = repo.create_shelf("Favorites", None)?;
        assert!(!shelf.pinned);

        let pinned = repo.set_shelf_pinned(shelf.id, true)?;
        assert!(pinned.pinned);

        let unpinned = repo.set_shelf_pinned(shelf.id, false)?;
        assert!(!unpinned.pinned);

        let err = repo.set_shelf_pinned(9999, true).unwrap_err();
        assert!(matches!(
            err,
            FicflowError::ShelfNotFound { shelf_id: 9999 }
        ));
        Ok(())
    }

    #[test]
    fn test_delete_shelf_excludes_from_list_but_keeps_fics() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic = fixtures::given_sample_fanfiction(1, "Forever Belongs Here");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let shelf = repo.create_shelf("Doomed", None)?;
        repo.add_fic_to_shelf(fic.id, shelf.id)?;

        repo.delete_shelf(shelf.id)?;

        // Shelf no longer appears.
        assert!(repo.list_shelves()?.is_empty());
        // get_shelf_by_id treats soft-deleted shelves as not found.
        assert!(matches!(
            repo.get_shelf_by_id(shelf.id).unwrap_err(),
            FicflowError::ShelfNotFound { .. }
        ));
        // The fic itself is still there.
        assert!(repo.list_fanfictions()?.iter().any(|f| f.id == fic.id));
        Ok(())
    }

    #[test]
    fn test_delete_shelf_returns_not_found_when_already_gone() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.delete_shelf(404).unwrap_err();
        assert!(matches!(err, FicflowError::ShelfNotFound { shelf_id: 404 }));
    }

    #[test]
    fn test_add_fic_to_shelf_creates_link() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic = fixtures::given_sample_fanfiction(11, "Test Fic");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let shelf = repo.create_shelf("Reading", None)?;

        repo.add_fic_to_shelf(fic.id, shelf.id)?;

        let in_shelf = repo.list_fics_in_shelf(shelf.id)?;
        assert_eq!(in_shelf.len(), 1);
        assert_eq!(in_shelf[0].id, fic.id);
        Ok(())
    }

    #[test]
    fn test_add_fic_to_shelf_is_idempotent() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic = fixtures::given_sample_fanfiction(12, "Twice-Added Fic");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let shelf = repo.create_shelf("Reading", None)?;

        repo.add_fic_to_shelf(fic.id, shelf.id)?;
        // Second add must not error and must not duplicate.
        repo.add_fic_to_shelf(fic.id, shelf.id)?;

        assert_eq!(repo.count_fics_in_shelf(shelf.id)?, 1);
        Ok(())
    }

    #[test]
    fn test_add_fic_to_shelf_rejects_missing_fic() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let shelf = repo.create_shelf("Reading", None)?;
        let err = repo.add_fic_to_shelf(9999, shelf.id).unwrap_err();
        assert!(matches!(err, FicflowError::NotFound { fic_id: 9999 }));
        Ok(())
    }

    #[test]
    fn test_add_fic_to_shelf_rejects_missing_shelf() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic = fixtures::given_sample_fanfiction(13, "Lonely Fic");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;

        let err = repo.add_fic_to_shelf(fic.id, 9999).unwrap_err();
        assert!(matches!(
            err,
            FicflowError::ShelfNotFound { shelf_id: 9999 }
        ));
        Ok(())
    }

    #[test]
    fn test_remove_fic_from_shelf() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic = fixtures::given_sample_fanfiction(14, "Removed Fic");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let shelf = repo.create_shelf("Reading", None)?;
        repo.add_fic_to_shelf(fic.id, shelf.id)?;

        repo.remove_fic_from_shelf(fic.id, shelf.id)?;

        assert!(repo.list_fics_in_shelf(shelf.id)?.is_empty());
        Ok(())
    }

    #[test]
    fn test_list_fics_in_shelf_sorted_by_title_excludes_deleted_fics() -> Result<(), Box<dyn Error>>
    {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic1 = fixtures::given_sample_fanfiction(21, "Banana Saga");
        let fic2 = fixtures::given_sample_fanfiction(22, "Apple Tale");
        let fic3 = fixtures::given_sample_fanfiction(23, "Cherry Story");
        fixtures::when_fanfiction_added_to_db(&conn, &fic1)?;
        fixtures::when_fanfiction_added_to_db(&conn, &fic2)?;
        fixtures::when_fanfiction_added_to_db(&conn, &fic3)?;
        let shelf = repo.create_shelf("Reading", None)?;
        repo.add_fic_to_shelf(fic1.id, shelf.id)?;
        repo.add_fic_to_shelf(fic2.id, shelf.id)?;
        repo.add_fic_to_shelf(fic3.id, shelf.id)?;

        // Soft-delete the middle one — it must drop out of the listing.
        repo.delete_fanfiction(fic3.id)?;

        let titles: Vec<String> = repo
            .list_fics_in_shelf(shelf.id)?
            .into_iter()
            .map(|f| f.title)
            .collect();
        assert_eq!(titles, vec!["Apple Tale", "Banana Saga"]);
        Ok(())
    }

    #[test]
    fn test_list_fics_in_shelf_rejects_missing_shelf() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.list_fics_in_shelf(9999).unwrap_err();
        assert!(matches!(
            err,
            FicflowError::ShelfNotFound { shelf_id: 9999 }
        ));
    }

    #[test]
    fn test_list_shelves_for_fic_sorted_excludes_deleted_shelves() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic = fixtures::given_sample_fanfiction(31, "Shared Fic");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let s_zebra = repo.create_shelf("Zebra", None)?;
        let s_apple = repo.create_shelf("Apple", None)?;
        let s_mango = repo.create_shelf("Mango", None)?;
        repo.add_fic_to_shelf(fic.id, s_zebra.id)?;
        repo.add_fic_to_shelf(fic.id, s_apple.id)?;
        repo.add_fic_to_shelf(fic.id, s_mango.id)?;

        // Soft-delete one shelf — it must drop out.
        repo.delete_shelf(s_mango.id)?;

        let names: Vec<String> = repo
            .list_shelves_for_fic(fic.id)?
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert_eq!(names, vec!["Apple", "Zebra"]);
        Ok(())
    }

    #[test]
    fn test_list_shelves_for_fic_rejects_missing_fic() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.list_shelves_for_fic(9999).unwrap_err();
        assert!(matches!(err, FicflowError::NotFound { fic_id: 9999 }));
    }

    #[test]
    fn test_count_fics_in_shelf_returns_active_link_count() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let shelf = repo.create_shelf("Reading", None)?;
        assert_eq!(repo.count_fics_in_shelf(shelf.id)?, 0, "empty shelf is 0");

        let fic1 = fixtures::given_sample_fanfiction(41, "First");
        let fic2 = fixtures::given_sample_fanfiction(42, "Second");
        fixtures::when_fanfiction_added_to_db(&conn, &fic1)?;
        fixtures::when_fanfiction_added_to_db(&conn, &fic2)?;
        repo.add_fic_to_shelf(fic1.id, shelf.id)?;
        repo.add_fic_to_shelf(fic2.id, shelf.id)?;

        assert_eq!(repo.count_fics_in_shelf(shelf.id)?, 2);
        Ok(())
    }

    #[test]
    fn test_count_fics_in_shelf_excludes_soft_deleted_fics() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let shelf = repo.create_shelf("Reading", None)?;
        let fic1 = fixtures::given_sample_fanfiction(51, "Stays");
        let fic2 = fixtures::given_sample_fanfiction(52, "Goes");
        fixtures::when_fanfiction_added_to_db(&conn, &fic1)?;
        fixtures::when_fanfiction_added_to_db(&conn, &fic2)?;
        repo.add_fic_to_shelf(fic1.id, shelf.id)?;
        repo.add_fic_to_shelf(fic2.id, shelf.id)?;
        assert_eq!(repo.count_fics_in_shelf(shelf.id)?, 2);

        // Soft-delete one of the fics; the COUNT query must skip it
        // because it joins on `fanfiction.deleted_at IS NULL`.
        repo.delete_fanfiction(fic2.id)?;

        assert_eq!(repo.count_fics_in_shelf(shelf.id)?, 1);
        Ok(())
    }

    #[test]
    fn test_count_fics_per_shelf_returns_only_non_empty_shelves() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        // Three shelves, but only two have any fics. The empty one
        // should be absent from the returned map (callers default
        // missing keys to 0).
        let s_full = repo.create_shelf("Full", None)?;
        let s_one = repo.create_shelf("One", None)?;
        let _s_empty = repo.create_shelf("Empty", None)?;
        let f1 = fixtures::given_sample_fanfiction(61, "F1");
        let f2 = fixtures::given_sample_fanfiction(62, "F2");
        let f3 = fixtures::given_sample_fanfiction(63, "F3");
        fixtures::when_fanfiction_added_to_db(&conn, &f1)?;
        fixtures::when_fanfiction_added_to_db(&conn, &f2)?;
        fixtures::when_fanfiction_added_to_db(&conn, &f3)?;
        repo.add_fic_to_shelf(f1.id, s_full.id)?;
        repo.add_fic_to_shelf(f2.id, s_full.id)?;
        repo.add_fic_to_shelf(f3.id, s_one.id)?;

        let counts = repo.count_fics_per_shelf()?;

        assert_eq!(counts.get(&s_full.id).copied(), Some(2));
        assert_eq!(counts.get(&s_one.id).copied(), Some(1));
        // The empty shelf doesn't appear in the map.
        assert!(!counts.contains_key(&_s_empty.id));
        Ok(())
    }

    #[test]
    fn test_count_fics_per_shelf_excludes_soft_deleted_fics() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let shelf = repo.create_shelf("Reading", None)?;
        let f1 = fixtures::given_sample_fanfiction(71, "Stays");
        let f2 = fixtures::given_sample_fanfiction(72, "Goes");
        fixtures::when_fanfiction_added_to_db(&conn, &f1)?;
        fixtures::when_fanfiction_added_to_db(&conn, &f2)?;
        repo.add_fic_to_shelf(f1.id, shelf.id)?;
        repo.add_fic_to_shelf(f2.id, shelf.id)?;
        repo.delete_fanfiction(f2.id)?;

        let counts = repo.count_fics_per_shelf()?;
        // Soft-deleted fic must not be counted.
        assert_eq!(counts.get(&shelf.id).copied(), Some(1));
        Ok(())
    }

    #[test]
    fn test_count_fics_per_shelf_empty_db_returns_empty_map() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let counts = repo.count_fics_per_shelf()?;
        assert!(counts.is_empty(), "no shelves → empty map");
        Ok(())
    }

    #[test]
    fn test_count_fics_in_shelf_rejects_missing_shelf() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.count_fics_in_shelf(9999).unwrap_err();
        assert!(matches!(
            err,
            FicflowError::ShelfNotFound { shelf_id: 9999 }
        ));
    }

    #[test]
    fn test_create_shelf_with_parent_sets_parent_id() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let parent = repo.create_shelf("Fantasy", None)?;
        let child = repo.create_shelf("Dragons", Some(parent.id))?;

        assert_eq!(child.parent_shelf_id, Some(parent.id));
        assert_eq!(
            repo.get_shelf_by_id(child.id)?.parent_shelf_id,
            Some(parent.id)
        );
        Ok(())
    }

    #[test]
    fn test_create_shelf_rejects_missing_parent() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.create_shelf("Orphan", Some(9999)).unwrap_err();
        assert!(matches!(
            err,
            FicflowError::ShelfNotFound { shelf_id: 9999 }
        ));
    }

    #[test]
    fn test_create_shelf_rejects_depth_beyond_three() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let l1 = repo.create_shelf("L1", None)?;
        let l2 = repo.create_shelf("L2", Some(l1.id))?;
        let l3 = repo.create_shelf("L3", Some(l2.id))?;

        let err = repo.create_shelf("L4", Some(l3.id)).unwrap_err();
        assert!(matches!(err, FicflowError::ShelfDepthExceeded { max: 3 }));
        Ok(())
    }

    #[test]
    fn test_move_shelf_nests_and_unnests() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let parent = repo.create_shelf("Fantasy", None)?;
        let child = repo.create_shelf("Dragons", None)?;

        let moved = repo.move_shelf(child.id, Some(parent.id))?;
        assert_eq!(moved.parent_shelf_id, Some(parent.id));

        let unnested = repo.move_shelf(child.id, None)?;
        assert_eq!(unnested.parent_shelf_id, None);
        Ok(())
    }

    #[test]
    fn test_move_shelf_rejects_self_and_descendant_cycles() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let parent = repo.create_shelf("Fantasy", None)?;
        let child = repo.create_shelf("Dragons", Some(parent.id))?;
        let grandchild = repo.create_shelf("Wyverns", Some(child.id))?;

        let err = repo.move_shelf(parent.id, Some(parent.id)).unwrap_err();
        assert!(matches!(err, FicflowError::ShelfCycle));

        let err = repo.move_shelf(parent.id, Some(grandchild.id)).unwrap_err();
        assert!(matches!(err, FicflowError::ShelfCycle));
        Ok(())
    }

    #[test]
    fn test_move_shelf_rejects_when_subtree_would_exceed_depth() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let root = repo.create_shelf("Root", None)?;
        let nested = repo.create_shelf("Nested", Some(root.id))?;
        let pair_top = repo.create_shelf("Pair Top", None)?;
        let _pair_leaf = repo.create_shelf("Pair Leaf", Some(pair_top.id))?;

        let err = repo.move_shelf(pair_top.id, Some(nested.id)).unwrap_err();
        assert!(matches!(err, FicflowError::ShelfDepthExceeded { max: 3 }));

        let moved = repo.move_shelf(pair_top.id, Some(root.id))?;
        assert_eq!(moved.parent_shelf_id, Some(root.id));
        Ok(())
    }

    #[test]
    fn test_move_shelf_rejects_missing_shelf_or_parent() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let shelf = repo.create_shelf("Lonely", None)?;

        let err = repo.move_shelf(9999, Some(shelf.id)).unwrap_err();
        assert!(matches!(
            err,
            FicflowError::ShelfNotFound { shelf_id: 9999 }
        ));

        let err = repo.move_shelf(shelf.id, Some(9999)).unwrap_err();
        assert!(matches!(
            err,
            FicflowError::ShelfNotFound { shelf_id: 9999 }
        ));
        Ok(())
    }

    #[test]
    fn test_delete_parent_promotes_children_to_grandparent() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let grandparent = repo.create_shelf("Fantasy", None)?;
        let parent = repo.create_shelf("Dragons", Some(grandparent.id))?;
        let child = repo.create_shelf("Wyverns", Some(parent.id))?;

        repo.delete_shelf(parent.id)?;

        assert_eq!(
            repo.get_shelf_by_id(child.id)?.parent_shelf_id,
            Some(grandparent.id)
        );
        Ok(())
    }

    #[test]
    fn test_delete_root_shelf_promotes_children_to_root() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let parent = repo.create_shelf("Fantasy", None)?;
        let child = repo.create_shelf("Dragons", Some(parent.id))?;

        repo.delete_shelf(parent.id)?;

        assert_eq!(repo.get_shelf_by_id(child.id)?.parent_shelf_id, None);
        Ok(())
    }

    #[test]
    fn test_list_fics_in_shelf_includes_descendants_distinct() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let parent = repo.create_shelf("Fantasy", None)?;
        let child = repo.create_shelf("Dragons", Some(parent.id))?;
        let grandchild = repo.create_shelf("Wyverns", Some(child.id))?;
        let f_parent = fixtures::given_sample_fanfiction(81, "On Parent");
        let f_both = fixtures::given_sample_fanfiction(82, "On Parent And Child");
        let f_deep = fixtures::given_sample_fanfiction(83, "On Grandchild");
        fixtures::when_fanfiction_added_to_db(&conn, &f_parent)?;
        fixtures::when_fanfiction_added_to_db(&conn, &f_both)?;
        fixtures::when_fanfiction_added_to_db(&conn, &f_deep)?;
        repo.add_fic_to_shelf(f_parent.id, parent.id)?;
        repo.add_fic_to_shelf(f_both.id, parent.id)?;
        repo.add_fic_to_shelf(f_both.id, child.id)?;
        repo.add_fic_to_shelf(f_deep.id, grandchild.id)?;

        let in_parent: Vec<u64> = repo
            .list_fics_in_shelf(parent.id)?
            .into_iter()
            .map(|f| f.id)
            .collect();
        assert_eq!(in_parent.len(), 3, "union of subtree, duplicates collapsed");
        assert!(in_parent.contains(&f_parent.id));
        assert!(in_parent.contains(&f_both.id));
        assert!(in_parent.contains(&f_deep.id));

        let in_child: Vec<u64> = repo
            .list_fics_in_shelf(child.id)?
            .into_iter()
            .map(|f| f.id)
            .collect();
        assert_eq!(in_child.len(), 2);
        assert!(in_child.contains(&f_both.id));
        assert!(in_child.contains(&f_deep.id));

        let in_grandchild: Vec<u64> = repo
            .list_fics_in_shelf(grandchild.id)?
            .into_iter()
            .map(|f| f.id)
            .collect();
        assert_eq!(in_grandchild, vec![f_deep.id]);
        Ok(())
    }

    #[test]
    fn test_count_fics_in_shelf_dedupes_fic_in_parent_and_child() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let parent = repo.create_shelf("Fantasy", None)?;
        let child = repo.create_shelf("Dragons", Some(parent.id))?;
        let fic = fixtures::given_sample_fanfiction(91, "Everywhere");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        repo.add_fic_to_shelf(fic.id, parent.id)?;
        repo.add_fic_to_shelf(fic.id, child.id)?;

        assert_eq!(repo.count_fics_in_shelf(parent.id)?, 1);
        Ok(())
    }

    #[test]
    fn test_count_fics_per_shelf_rolls_up_to_ancestors() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let parent = repo.create_shelf("Fantasy", None)?;
        let child = repo.create_shelf("Dragons", Some(parent.id))?;
        let f1 = fixtures::given_sample_fanfiction(95, "Direct");
        let f2 = fixtures::given_sample_fanfiction(96, "Nested");
        fixtures::when_fanfiction_added_to_db(&conn, &f1)?;
        fixtures::when_fanfiction_added_to_db(&conn, &f2)?;
        repo.add_fic_to_shelf(f1.id, parent.id)?;
        repo.add_fic_to_shelf(f2.id, child.id)?;

        let counts = repo.count_fics_per_shelf()?;
        assert_eq!(counts.get(&parent.id).copied(), Some(2));
        assert_eq!(counts.get(&child.id).copied(), Some(1));
        Ok(())
    }
}
