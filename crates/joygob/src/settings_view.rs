//! Settings page: horizontal tabs, one group of options per tab.

use eframe::egui::{self, RichText, Vec2};
use gob_i18n::I18n;

use crate::settings::{Settings, Theme};
use crate::style::{self, icon};

const REPO_URL: &str = "https://github.com/h0useRus/JoystickGobloon";
const DONATE_URL: &str = "https://www.paypal.com/paypalme/h0userus";
/// The author's name is not translated.
const AUTHOR: &str = "Denis Ibragimov";

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
    /// The tray icon was switched on or off.
    Tray,
    /// Start with Windows was switched: update the registry.
    Autostart(bool),
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
        SettingsTab::Appearance => {
            let change = appearance(ui, i18n, settings);
            ui.add_space(18.0);
            tray_section(ui, i18n, settings).or(change)
        }
        SettingsTab::General => {
            let reopen = ui.checkbox(&mut settings.reopen_last, i18n.tr("settings-reopen-last"));
            let whitelist = ui.checkbox(
                &mut settings.hidhide_whitelist_self,
                i18n.tr("settings-hidhide-self"),
            );
            ui.add_space(8.0);
            let autostart = ui.checkbox(
                &mut settings.start_with_windows,
                i18n.tr("settings-autostart"),
            );
            let activate = ui.checkbox(
                &mut settings.activate_on_start,
                i18n.tr("settings-activate-on-start"),
            );
            let changed = [&reopen, &whitelist, &autostart, &activate]
                .iter()
                .any(|r| r.changed());
            if changed {
                settings.save();
            }
            if autostart.changed() {
                Some(Change::Autostart(settings.start_with_windows))
            } else {
                None
            }
        }
        SettingsTab::About => {
            about(ui, i18n, logo);
            None
        }
    }
}

/// Tray icon options (Appearance tab).
fn tray_section(ui: &mut egui::Ui, i18n: &I18n, settings: &mut Settings) -> Option<Change> {
    ui.label(
        RichText::new(format!(
            "{}  {}",
            icon::TRAY,
            i18n.tr("settings-tray-title")
        ))
        .strong(),
    );
    ui.add_space(4.0);
    let tray = ui.checkbox(&mut settings.show_tray, i18n.tr("settings-tray"));
    let close = ui.add_enabled(
        settings.show_tray,
        egui::Checkbox::new(
            &mut settings.close_to_tray,
            i18n.tr("settings-close-to-tray"),
        ),
    );
    if tray.changed() || close.changed() {
        settings.save();
    }
    tray.changed().then_some(Change::Tray)
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
    let accent = style::accent(ui);
    ui.add_space(12.0);
    ui.vertical_centered(|ui| {
        ui.set_max_width(480.0);
        ui.add(egui::Image::new(logo).fit_to_exact_size(Vec2::splat(112.0)));
        ui.add_space(8.0);
        ui.label(RichText::new(crate::APP_NAME).size(26.0).strong());
        // Version chip; alpha builds (x.y.z-alpha.n) are marked.
        let alpha = crate::VERSION.contains('-');
        // A non-interactive button: frames stretch to the column width here.
        let text = if alpha {
            format!("v{}  ·  {}", crate::VERSION, i18n.tr("about-alpha"))
        } else {
            format!("v{}", crate::VERSION)
        };
        let fill = if alpha {
            ui.visuals().warn_fg_color.gamma_multiply(0.18)
        } else {
            accent.gamma_multiply(0.18)
        };
        ui.add(
            egui::Button::new(RichText::new(text).small().strong())
                .fill(fill)
                .stroke(egui::Stroke::NONE)
                .corner_radius(10)
                .sense(egui::Sense::hover()),
        );
        ui.add_space(10.0);
        ui.label(RichText::new(i18n.tr("about-text")).size(15.0));
        ui.add_space(6.0);
        ui.label(RichText::new(i18n.trf("about-created", &[("name", AUTHOR.into())])).weak());

        ui.add_space(16.0);
        // Support: the one accented call to action.
        let support = egui::Button::new(
            RichText::new(format!("{}  {}", icon::HEART, i18n.tr("about-support")))
                .size(15.0)
                .color(ui.visuals().strong_text_color()),
        )
        .fill(accent.gamma_multiply(0.45))
        .stroke(egui::Stroke::new(1.0, accent))
        .corner_radius(style::RADIUS)
        .min_size(Vec2::new(220.0, 36.0));
        if ui
            .add(support)
            .on_hover_text(i18n.tr("about-support-tip"))
            .clicked()
        {
            ui.ctx().open_url(egui::OpenUrl::new_tab(DONATE_URL));
        }
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            // Center the row of links under the button.
            let links = [
                (icon::GITHUB_LOGO, "about-source", REPO_URL.to_string()),
                (icon::BUG, "about-issues", format!("{REPO_URL}/issues")),
                (
                    icon::DOWNLOAD_SIMPLE,
                    "about-releases",
                    format!("{REPO_URL}/releases"),
                ),
            ];
            let width: f32 = links
                .iter()
                .map(|(g, k, _)| {
                    let text = format!("{g}  {}", i18n.tr(k));
                    ui.fonts(|f| {
                        f.layout_no_wrap(text, egui::TextStyle::Body.resolve(ui.style()), accent)
                            .size()
                            .x
                    }) + 24.0
                })
                .sum();
            ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
            for (glyph, key, url) in links {
                ui.hyperlink_to(format!("{glyph}  {}", i18n.tr(key)), url);
                ui.add_space(12.0);
            }
        });

        ui.add_space(20.0);
        ui.separator();
        ui.add_space(6.0);
        ui.label(RichText::new(i18n.tr("about-credits")).weak());
        ui.label(RichText::new(i18n.tr("about-license")).weak());
    });
}
