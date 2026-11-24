#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ficflow::infrastructure::config::Config;
    use ficflow::interfaces::gui::{AppConfig, ColumnKey, ThemeChoice};

    const LEGACY_CONFIG: &str = r#"
        visible_columns = ["Title", "Words"]
        window_maximized = true
        window_fullscreen = false
        text_zoom = 1.5
        theme = "Dark"
        library_path = "/books/fanfictions.db"

        [default_sort]
        column = "Title"
        direction = "Ascending"

        [last_view]
        Shelf = 3
    "#;

    #[test]
    fn existing_flat_config_file_still_loads() {
        let cfg: AppConfig = toml::from_str(LEGACY_CONFIG).unwrap();

        assert_eq!(
            cfg.visible_columns,
            vec![ColumnKey::Title, ColumnKey::Words]
        );
        assert_eq!(cfg.theme, ThemeChoice::Dark);
        assert_eq!(cfg.text_zoom, 1.5);
        assert!(cfg.window_maximized);
        assert_eq!(
            cfg.shared.library_path,
            Some(PathBuf::from("/books/fanfictions.db"))
        );

        let shared: Config = toml::from_str(LEGACY_CONFIG).unwrap();
        assert_eq!(
            shared.library_path,
            Some(PathBuf::from("/books/fanfictions.db"))
        );
    }

    #[test]
    fn gui_config_round_trip_keeps_library_path_flat() {
        let cfg = AppConfig {
            shared: Config {
                library_path: Some(PathBuf::from("/books/fanfictions.db")),
                ..Config::default()
            },
            ..AppConfig::default()
        };

        let text = toml::to_string_pretty(&cfg).unwrap();
        let table: toml::Table = toml::from_str(&text).unwrap();

        assert_eq!(
            table.get("library_path").and_then(|v| v.as_str()),
            Some("/books/fanfictions.db")
        );
        assert!(!table.contains_key("shared"));
    }
}
