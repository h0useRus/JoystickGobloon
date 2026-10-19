//! Settings page: horizontal tabs, one group of options per tab.

use eframe::egui::{self, RichText, Vec2};
use gob_i18n::I18n;

use crate::leds::{LedDevice, LedValue};
use crate::settings::{LedDefault, Settings, Theme};
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
    Lighting,
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
    /// Light these LEDs of an attached device now (a test).
    PreviewLeds {
        device: String,
        leds: Vec<LedValue>,
    },
    /// Default LED colors were edited or experimental features switched.
    LedDefaults,
}

pub fn show(
    ui: &mut egui::Ui,
    i18n: &I18n,
    settings: &mut Settings,
    tab: &mut SettingsTab,
    logo: &egui::TextureHandle,
    updater: &mut crate::update::Updater,
    led_devices: &[LedDevice],
) -> Option<Change> {
    if !settings.experimental && *tab == SettingsTab::Lighting {
        *tab = SettingsTab::General;
    }
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
            (SettingsTab::Lighting, icon::LIGHTBULB, "settings-lighting"),
            (SettingsTab::About, icon::INFO, "settings-about"),
        ] {
            if t == SettingsTab::Lighting && !settings.experimental {
                continue;
            }
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
            let reopen = ui.add(style::switch(
                &mut settings.reopen_last,
                i18n.tr("settings-reopen-last"),
            ));
            let whitelist = ui.add(style::switch(
                &mut settings.hidhide_whitelist_self,
                i18n.tr("settings-hidhide-self"),
            ));
            ui.add_space(8.0);
            let autostart = ui.add(style::switch(
                &mut settings.start_with_windows,
                i18n.tr("settings-autostart"),
            ));
            let activate = ui.add(style::switch(
                &mut settings.activate_on_start,
                i18n.tr("settings-activate-on-start"),
            ));
            let updates = ui.add(style::switch(
                &mut settings.check_updates,
                i18n.tr("settings-check-updates"),
            ));
            ui.add_space(8.0);
            let experimental = ui
                .add(style::switch(
                    &mut settings.experimental,
                    i18n.tr("settings-experimental"),
                ))
                .on_hover_text(i18n.tr("settings-experimental-tip"));
            let changed = [
                &reopen,
                &whitelist,
                &autostart,
                &activate,
                &updates,
                &experimental,
            ]
            .iter()
            .any(|r| r.changed());
            if changed {
                settings.save();
            }
            if autostart.changed() {
                Some(Change::Autostart(settings.start_with_windows))
            } else if experimental.changed() {
                Some(Change::LedDefaults)
            } else {
                None
            }
        }
        SettingsTab::Lighting => lighting(ui, i18n, settings, led_devices),
        SettingsTab::About => {
            about(ui, i18n, logo, updater);
            None
        }
    }
}

/// Default LED colors per attached joystick with LED control.
fn lighting(
    ui: &mut egui::Ui,
    i18n: &I18n,
    settings: &mut Settings,
    led_devices: &[LedDevice],
) -> Option<Change> {
    ui.label(RichText::new(i18n.tr("settings-leds-hint")).weak());
    if led_devices.is_empty() {
        ui.add_space(8.0);
        ui.label(i18n.tr("settings-leds-none"));
        return None;
    }
    let mut change = None;
    let mut edited = false;
    for d in led_devices {
        let (guid, vkb) = (d.guid.clone(), d.vkb);
        let leds: Vec<LedValue> = settings
            .leds
            .get(&guid)
            .map(|e| e.led.iter().map(LedDefault::value).collect())
            .unwrap_or_default();
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}  {}", icon::JOYSTICK, d.name)).strong());
            if style::icon_button(ui, !leds.is_empty(), icon::PLAY, &i18n.tr("edit-led-test"))
                .clicked()
            {
                change = Some(Change::PreviewLeds {
                    device: guid.clone(),
                    leds: leds.clone(),
                });
            }
            if style::icon_button(ui, true, icon::PLUS_CIRCLE, &i18n.tr("settings-leds-add"))
                .clicked()
            {
                // The next known LED not listed yet, else the next number.
                let used =
                    |id: u8, element: u8| leds.iter().any(|l| (l.id, l.element) == (id, element));
                let (id, element) = gob_lighting::vkb::KNOWN_LEDS
                    .iter()
                    .filter(|_| vkb)
                    .map(|k| (k.id, k.element))
                    .find(|&(id, element)| !used(id, element))
                    .unwrap_or_else(|| (leds.iter().map(|l| l.id).max().map_or(0, |m| m + 1), 0));
                let entry = settings.leds.entry(guid.clone()).or_default();
                entry.name = d.name.clone();
                let mut value = LedValue {
                    id,
                    element,
                    color: [0, 0xFF, 0xFF],
                    ..Default::default()
                };
                crate::leds::adapt(&mut value, vkb);
                entry.led.push(LedDefault::from_value(value));
                edited = true;
            }
        });
        let mut remove = None;
        for (i, mut v) in leds.iter().copied().enumerate() {
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                if crate::leds::led_widgets(ui, i18n, ("default", guid.as_str(), i), vkb, &mut v) {
                    if let Some(e) = settings.leds.get_mut(&guid) {
                        e.led[i] = LedDefault::from_value(v);
                        edited = true;
                    }
                }
                if style::icon_button(ui, true, icon::PLAY, &i18n.tr("edit-led-test")).clicked() {
                    change = Some(Change::PreviewLeds {
                        device: guid.clone(),
                        leds: vec![v],
                    });
                }
                if style::icon_button(ui, true, icon::TRASH, &i18n.tr("edit-remove")).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            if let Some(e) = settings.leds.get_mut(&guid) {
                e.led.remove(i);
                if e.led.is_empty() {
                    settings.leds.remove(&guid);
                }
                edited = true;
            }
        }
    }
    if edited {
        settings.save();
        change = change.or(Some(Change::LedDefaults));
    }
    change
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
    let tray = ui.add(style::switch(
        &mut settings.show_tray,
        i18n.tr("settings-tray"),
    ));
    let close = ui.add_enabled(
        settings.show_tray,
        style::switch(
            &mut settings.close_to_tray,
            i18n.tr("settings-close-to-tray"),
        ),
    );
    let minimize = ui.add_enabled(
        settings.show_tray,
        style::switch(
            &mut settings.minimize_to_tray,
            i18n.tr("settings-minimize-to-tray"),
        ),
    );
    if tray.changed() || close.changed() || minimize.changed() {
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
            (Theme::System, icon::MONITOR, "theme-system"),
        ] {
            let selected = settings.theme == theme;
            let text = RichText::new(format!("{glyph}  {}", i18n.tr(key)));
            let button = egui::Button::new(text)
                .selected(selected)
                .corner_radius(style::RADIUS)
                .min_size(Vec2::new(96.0, 30.0));
            let tip = if theme == Theme::System {
                i18n.tr("theme-system-tip")
            } else {
                i18n.tr(key)
            };
            if ui.add(button).on_hover_text(tip).clicked() && !selected {
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

fn about(
    ui: &mut egui::Ui,
    i18n: &I18n,
    logo: &egui::TextureHandle,
    updater: &mut crate::update::Updater,
) {
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
        // Update check, centered under the version.
        ui.add_space(6.0);
        ui.scope_builder(
            egui::UiBuilder::new().layout(egui::Layout::top_down(egui::Align::Center)),
            |ui| updater.ui(ui, i18n),
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
        .stroke(egui::Stroke::new(1.0_f32, accent))
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
