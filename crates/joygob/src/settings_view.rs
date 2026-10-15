//! Settings page: horizontal tabs, one group of options per tab.

use eframe::egui::{self, RichText, Vec2};
use gob_i18n::I18n;

use crate::settings::{Settings, Theme};
use crate::style::{self, icon};

const REPO_URL: &str = "https://github.com/h0useRus/JoystickGobloon";

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Appearance,
    General,
    About,
}

/// Changes that need more than a field update (re-styling, reloading strings).
pub enum Change {
    Theme(Theme),
    Language(String),
}

pub fn show(
    ui: &mut egui::Ui,
    i18n: &I18n,
    settings: &mut Settings,
    tab: &mut SettingsTab,
    logo: &egui::TextureHandle,
) -> Option<Change> {
    ui.horizontal(|ui| {
        for (t, glyph, key) in [
            (
                SettingsTab::Appearance,
                icon::PALETTE,
                "settings-appearance",
            ),
            (
                SettingsTab::General,
                icon::SLIDERS_HORIZONTAL,
                "settings-general",
            ),
            (SettingsTab::About, icon::INFO, "settings-about"),
        ] {
            if style::tab(ui, *tab == t, glyph, &i18n.tr(key)).clicked() {
                *tab = t;
            }
        }
    });
    ui.separator();
    ui.add_space(8.0);

    match tab {
        SettingsTab::Appearance => appearance(ui, i18n, settings),
        SettingsTab::General => {
            let reopen = ui.checkbox(&mut settings.reopen_last, i18n.tr("settings-reopen-last"));
            let whitelist = ui.checkbox(
                &mut settings.hidhide_whitelist_self,
                i18n.tr("settings-hidhide-self"),
            );
            if reopen.changed() || whitelist.changed() {
                settings.save();
            }
            None
        }
        SettingsTab::About => {
            about(ui, i18n, logo);
            None
        }
    }
}

fn appearance(ui: &mut egui::Ui, i18n: &I18n, settings: &Settings) -> Option<Change> {
    let mut change = None;
    ui.horizontal(|ui| {
        for (theme, glyph, key) in [
            (Theme::Light, icon::SUN, "theme-light"),
            (Theme::Dark, icon::MOON, "theme-dark"),
        ] {
            let selected = settings.theme == theme;
            let mut job = egui::text::LayoutJob {
                halign: egui::Align::Center,
                ..Default::default()
            };
            let style = ui.style();
            RichText::new(glyph).size(30.0).append_to(
                &mut job,
                style,
                egui::FontSelection::Default,
                egui::Align::Center,
            );
            RichText::new(format!("\n{}", i18n.tr(key))).append_to(
                &mut job,
                style,
                egui::FontSelection::Default,
                egui::Align::Center,
            );
            let card = egui::Button::new(job)
                .selected(selected)
                .corner_radius(style::RADIUS)
                .min_size(Vec2::new(120.0, 80.0));
            if ui.add(card).clicked() && !selected {
                change = Some(Change::Theme(theme));
            }
        }
    });
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon::GLOBE).size(style::ICON_SIZE))
            .on_hover_text(i18n.tr("settings-language"));
        let languages = i18n.languages();
        let current = languages
            .iter()
            .find(|(code, _)| code == i18n.language())
            .map_or_else(String::new, |(_, name)| name.clone());
        egui::ComboBox::from_id_salt("language")
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (code, name) in &languages {
                    if ui.selectable_label(code == i18n.language(), name).clicked() {
                        change = Some(Change::Language(code.clone()));
                    }
                }
            });
    });
    change
}

fn about(ui: &mut egui::Ui, i18n: &I18n, logo: &egui::TextureHandle) {
    ui.horizontal(|ui| {
        ui.add(egui::Image::new(logo).fit_to_exact_size(Vec2::splat(96.0)));
        ui.vertical(|ui| {
            ui.heading("Joystick Gobloon");
            ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
            ui.add_space(4.0);
            ui.label(i18n.tr("about-text"));
            ui.add_space(4.0);
            ui.hyperlink_to(format!("{}  GitHub", icon::GITHUB_LOGO), REPO_URL);
        });
    });
}
