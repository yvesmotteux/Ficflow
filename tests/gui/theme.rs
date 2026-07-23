#[cfg(test)]
mod tests {
    use ficflow::interfaces::gui::{AppConfig, ThemeChoice};

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    fn given_harness() -> GuiHarness {
        let (conn, db_path, td) = fixtures::given_test_database();
        GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td)
    }

    #[test]
    fn default_theme_is_system() {
        assert_eq!(AppConfig::default().theme, ThemeChoice::System);
    }

    #[test]
    fn config_without_theme_key_falls_back_to_system() {
        let text = r#"
            visible_columns = ["Title"]

            [default_sort]
            column = "Updated"
            direction = "Descending"
        "#;

        let cfg: AppConfig = toml::from_str(text).unwrap();

        assert_eq!(cfg.theme, ThemeChoice::System);
    }

    #[test]
    fn theme_choice_survives_config_round_trip() {
        let cfg = AppConfig {
            theme: ThemeChoice::Dark,
            ..AppConfig::default()
        };

        let text = toml::to_string_pretty(&cfg).unwrap();
        let reloaded: AppConfig = toml::from_str(&text).unwrap();

        assert_eq!(reloaded.theme, ThemeChoice::Dark);
    }

    #[test]
    fn clear_theme_applies_to_context_and_survives_restart() {
        let mut h = given_harness();
        let ctx = h.ctx.clone();

        h.app.set_theme(&ctx, ThemeChoice::Clear);
        h.step();

        assert_eq!(h.ctx.theme(), egui::Theme::Light);

        h.ctx.set_theme(egui::ThemePreference::System);
        h.restart(vec!["http://127.0.0.1:1".into()]);

        assert_eq!(h.app.theme_choice(), ThemeChoice::Clear);
        assert_eq!(
            h.ctx.options(|o| o.theme_preference),
            egui::ThemePreference::Light
        );
    }

    #[test]
    fn system_theme_sets_system_preference() {
        let mut h = given_harness();
        let ctx = h.ctx.clone();

        h.app.set_theme(&ctx, ThemeChoice::Dark);
        h.app.set_theme(&ctx, ThemeChoice::System);
        h.restart(vec!["http://127.0.0.1:1".into()]);

        assert_eq!(h.app.theme_choice(), ThemeChoice::System);
        assert_eq!(
            h.ctx.options(|o| o.theme_preference),
            egui::ThemePreference::System
        );
    }
}
