#[cfg(test)]
mod tests {
    use ficflow::interfaces::gui::{AppConfig, ColumnKey};

    fn config_with(columns: &[ColumnKey]) -> AppConfig {
        AppConfig {
            visible_columns: columns.to_vec(),
            ..AppConfig::default()
        }
    }

    #[test]
    fn reorder_moves_column_right_after_target() {
        let mut cfg = config_with(&[ColumnKey::Title, ColumnKey::Author, ColumnKey::Status]);

        cfg.reorder_visible_column(ColumnKey::Title, ColumnKey::Status, true);

        assert_eq!(
            cfg.visible_columns,
            vec![ColumnKey::Author, ColumnKey::Status, ColumnKey::Title]
        );
    }

    #[test]
    fn reorder_moves_column_left_before_target() {
        let mut cfg = config_with(&[ColumnKey::Title, ColumnKey::Author, ColumnKey::Status]);

        cfg.reorder_visible_column(ColumnKey::Status, ColumnKey::Title, false);

        assert_eq!(
            cfg.visible_columns,
            vec![ColumnKey::Status, ColumnKey::Title, ColumnKey::Author]
        );
    }

    #[test]
    fn reorder_inserts_before_right_neighbour() {
        let mut cfg = config_with(&[ColumnKey::Title, ColumnKey::Author, ColumnKey::Status]);

        cfg.reorder_visible_column(ColumnKey::Title, ColumnKey::Status, false);

        assert_eq!(
            cfg.visible_columns,
            vec![ColumnKey::Author, ColumnKey::Title, ColumnKey::Status]
        );
    }

    #[test]
    fn reorder_onto_itself_is_a_noop() {
        let mut cfg = config_with(&[ColumnKey::Title, ColumnKey::Author]);

        cfg.reorder_visible_column(ColumnKey::Title, ColumnKey::Title, true);

        assert_eq!(
            cfg.visible_columns,
            vec![ColumnKey::Title, ColumnKey::Author]
        );
    }

    #[test]
    fn reorder_with_hidden_column_is_a_noop() {
        let mut cfg = config_with(&[ColumnKey::Title, ColumnKey::Author]);

        cfg.reorder_visible_column(ColumnKey::Words, ColumnKey::Title, true);
        cfg.reorder_visible_column(ColumnKey::Title, ColumnKey::Words, true);

        assert_eq!(
            cfg.visible_columns,
            vec![ColumnKey::Title, ColumnKey::Author]
        );
    }

    #[test]
    fn custom_column_order_survives_config_round_trip() {
        let cfg = config_with(&[
            ColumnKey::Updated,
            ColumnKey::Rating,
            ColumnKey::Title,
            ColumnKey::Author,
        ]);

        let text = toml::to_string_pretty(&cfg).unwrap();
        let reloaded: AppConfig = toml::from_str(&text).unwrap();

        assert_eq!(reloaded.visible_columns, cfg.visible_columns);
    }
}
