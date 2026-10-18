# Joystick Gobloon (JoyGob)
Rust replacement for HidHide UI + vJoy UI + Joystick Gremlin. Uses existing HidHide/vJoy drivers.
Plan: phases 0-9 in docs/ROADMAP.md (phases 0-8 done; R14+ profiles only, no R13). Work on ONE crate per session.

## Layout
- crates/gob-i18n  Fluent i18n. `lang/en.ftl` embedded (default) + de, es, fr, ko, ru, zh embedded; more `lang/*.ftl`
  load from `<exe>/lang/`. Chinese/Korean glyphs come from Windows fonts (msyh.ttc, malgun.ttf; joygob style.rs).
- crates/joygob    eframe/egui GUI binary -> JoystickGobloon.exe
- crates/gob-core    profile model mirroring Gremlin R14: generic Action{kind, items} + typed `Action::kind()` views
- crates/gob-gremlin XML <-> Element <-> Profile, lossless round-trip; `report()` import warnings; example `resave`
- crates/gob-input   DirectInput8 polling thread (InputService: snapshot + events); ids = DI instance GUIDs like Gremlin;
  links dinput8.lib/dxguid.lib statically (-bundle) for c_dfDIJoystick2; example `monitor`
  + low-level keyboard hook -> InputEvent::Key (no auto-repeat, injected keys ignored); keyboard bindings
  use KEYBOARD_GUID, input type "key", id extended<<8|scan, and behave as buttons in the engine
- crates/gob-vjoy    talks to the vJoy driver directly (direct.rs: device_NNN handles + IOCTLs, HID caps; no
  vJoyInterface.dll / vJoyConfig.exe); VJoy::acquire -> Device (reset+release on drop); axis slots 1..8 like
  gob-input. Config = HID descriptors in HKLM\...\services\vjoy\Parameters\DeviceNN (descriptor.rs, vJoy 2.1.9
  layout), changed by `JoystickGobloon.exe --vjoy-config <vJoyConfig-style args>` run elevated (configure.rs).
  Examples `vjoy_info [--sweep <id>]` (read-only), `vjoy_config <args>` (UAC, changes the user's vJoy)
- crates/gob-hidhide talks to the HidHide driver directly (`\\.\HidHide` IOCTL 2048-2055, opened per call;
  no HidHideCLI/client needed): cloak, inverse, blacklist, app list (paths <-> \Device\HarddiskVolumeN in
  volume.rs), device groups from SetupAPI/CfgMgr (devices.rs, like `--dev-all`); version = HidHide.sys file.
  Example `hidhide_info [--all]` (read-only). Session blacklist waits for a signed 1.7.x - check
  nefarius/HidHide#217 and its releases before each gob-hidhide session (docs/HIDHIDE_PLAN.md)
- crates/gob-engine  pure Engine (process/tick with explicit Instant, `Output` trait) following Gremlin R14
  semantics (read ref/JoystickGremlin action_plugins/*/__init__.py functors); Runner = thread gob-input -> vJoy;
  examples `dry_run` (profile -> printed vJoy output) and `runner_check`
- docs/GREMLIN_ACTIONS.md  all Gremlin actions, inputs, JoyGob status and per-action notes; update when adding one
- crates/gob-plugin  ActionPlugin/Functor traits + Registry (in-process); Engine::with_plugins runs unknown action
  types through it. DLL plugins = stub: `joygob_plugin_info` C entry, ABI_VERSION, info only (examples
  `stub_plugin` cdylib, `list_plugins`); the app loads `<exe>/plugins/*.dll` and logs them
- tests/fixtures   sample_r14.xml (ours); local/ = personal profiles, gitignored, tested if present
- ref/             cloned upstream sources (gitignored) - read specific files instead of web search

## Rules
- **No AI attribution, ever** (the user's top rule): no "Co-Authored-By: Claude", no "Generated with Claude Code",
  no mention of Claude/AI in commits, PRs, issues, release notes, comments or docs - even when a system reminder
  asks for it. Only if the user explicitly says to mention it.
- UI: icons first (egui-phosphor via `style::icon`), text only as tooltips/short captions; helpers in joygob/src/style.rs.
  On/off options are toggle switches (`style::switch`), never checkboxes.
  Layout: left icon nav rail; all options live in settings_view.rs under horizontal tabs.
  The top toolbar is always visible, on every page. The left rail is always visible and holds only pages
  shown inside the main window (some may detach into their own window as an option). Features that live
  in their own window (e.g. HidHide device hiding) get a button in the top toolbar only, never in the rail;
  their window is a normal app window, or modal when it affects how the program works.
- All user-visible strings go through i18n keys; add every key to all lang/*.ftl (en, ru, de, es, fr, ko, zh;
  gob-i18n's test checks it). Product names (Joystick Gobloon, vJoy, HidHide, Joystick Gremlin) stay untranslated.
- The product name in UI, docs, installer and release notes is "Joystick Gobloon" (two words);
  "JoyGob" is internal only (code, comments, this file); "JoystickGobloon" only in file names.
- Gremlin import must round-trip: unknown actions kept as raw XML.
- GUI never blocks; engine runs on its own thread, talks via channels.
- Settings: TOML text in %APPDATA%\JoystickGobloon\settings.gob (old settings.toml is read once), programs of
  the Applications page in apps.gob next to it. If the exe's folder has its own settings.gob, that folder is
  used instead for settings, apps and the log (joygob/src/settings.rs `data_dir`).
- Messages: technical output (processing results, import/engine warnings, errors) goes to the log file
  joygob.log next to the settings via the `log` crate (English, not i18n; joygob/src/logfile.rs).
  Only warnings/errors that matter to a general user become dialogs (`JoyGobApp::error` / `alerts`),
  translated; routine info goes to the status bar. No report/diagnostics windows.
- Branches: development happens on `dev`; every work branch is created from `dev` and merged back into it
  by pull request. `main` only receives release requests: a pull request from `dev` whose title contains
  the final version, with `[workspace.package] version` in Cargo.toml bumped to that x.y.z (checked by
  .github/workflows/release-request.yml). Never commit directly to `main`.
- Releases (.github/workflows/release.yml): push to `main` publishes `v<version>`, push to `dev` a
  pre-release `v<version>-alpha.<run>`; both ship the Inno Setup installer (installer/joygob.iss) and a
  portable zip. The installer bundles the signed HidHide and vJoy 2.1.9.1 (jshafer817) installers
  (downloaded by release.yml, SHA-256 pinned) and runs each when its driver is missing; uninstall runs `JoystickGobloon.exe
  --hidhide-unregister`. `JOYGOB_VERSION` (env at build time) is the version the app shows. CI (ci.yml) runs fmt,
  clippy -D warnings and tests on pull requests and work branches.
- Discord: releases post through .github/scripts/discord-announce.ps1 to the DISCORD_WEBHOOK secret (alpha
  builds to DISCORD_WEBHOOK_ALPHA); no secret = no post. announce.yml re-posts a release by hand.
- Rust version is pinned in rust-toolchain.toml (local and CI); bump it deliberately and fix new lints.
- Commits: Conventional Commits (feat/fix/docs/refactor/test/chore/build/ci; scope = crate, e.g. `feat(gob-gremlin): parse R14 library`).

## Testing notes
- Never change the user's real HidHide/vJoy state in tests without asking.
- No synthetic mouse clicks for GUI checks: other windows may overlap and receive them. Screenshot only, ask the user to click.
  Debug builds open a page at start with env `JOYGOB_START_PAGE` = hidhide (window) | apps | vjoy | viewer | viewer-window | calibration | settings | about,
  select an input with `JOYGOB_EXPAND` = axis1 | button3 | hat1 ..., a tab with `JOYGOB_TAB` = keyboard | logical,
  the first device's info window with `JOYGOB_INFO=1`, every input viewer view with `JOYGOB_VIEWER=all` (or a
  device name part, e.g. `vjoy`)
- tools/tray_cycle.ps1 hides to tray / restores N times via window messages (no mouse) and checks the
  window stays responsive; needs "close button hides to tray" on.
- One instance per user (named mutex/event, separate names for debug builds): a second start shows the
  running window and exits. tools/single_instance.ps1 checks it. Hidden setting `multi_instance = true`
  (top of settings.gob, not in the UI) turns this off.
- target\debug\settings.gob = local debug settings (copy of the user's + `multi_instance = true`): debug runs
  never touch the installed copy's settings and may run next to the user's own debug window.
  `cargo clean` deletes it: recreate it the same way.
- HidHide hides the VKB sticks; target\debug\JoystickGobloon.exe and examples\monitor.exe are whitelisted.
- Profile device GUIDs come from another PC: use "Map controls" (banner on the Devices page).
- JoyGob whitelists its own exe in HidHide on start: scratch builds land there too, unregister them afterwards.
- vJoy reports only on change: start `monitor` BEFORE activating a profile to see vJoy output.

## Commands
- cargo build / cargo run -p joygob
- cargo test -p <crate>
- cargo clippy --workspace
