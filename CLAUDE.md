# Joystick Gobloon (JoyGob)
Rust replacement for HidHide UI + vJoy UI + Joystick Gremlin. Uses existing HidHide/vJoy drivers.
Plan: phases 0-10 in docs/ROADMAP.md (phases 0-6 done; device substitution added). Work on ONE crate per session.

## Layout
- crates/gob-i18n  Fluent i18n. `lang/en.ftl` embedded (default), other `lang/*.ftl` embedded or loaded from `<exe>/lang/`.
- crates/joygob    eframe/egui GUI binary -> JoystickGobloon.exe
- crates/gob-core    profile model mirroring Gremlin R14: generic Action{kind, items} + typed `Action::kind()` views
- crates/gob-gremlin XML <-> Element <-> Profile, lossless round-trip; `report()` import warnings; example `resave`
- crates/gob-input   DirectInput8 polling thread (InputService: snapshot + events); ids = DI instance GUIDs like Gremlin;
  links dinput8.lib/dxguid.lib statically (-bundle) for c_dfDIJoystick2; example `monitor`
- crates/gob-vjoy    runtime-loaded vJoyInterface.dll (path from vJoy uninstall key); VJoy::acquire -> Device
  (reset+release on drop); axis slots 1..8 like gob-input; example `vjoy_info --sweep <id>`
- crates/gob-hidhide wraps HidHideCLI.exe (path from HKLM\SOFTWARE\Nefarius Software Solutions e.U.\HidHide):
  cloak, hidden devices (HID + USB container), app whitelist, `--dev-gaming` JSON; example `hidhide_info`
- crates/gob-engine  pure Engine (process/tick with explicit Instant, `Output` trait) following Gremlin R14
  semantics (read ref/JoystickGremlin action_plugins/*/__init__.py functors); Runner = thread gob-input -> vJoy;
  examples `dry_run` (profile -> printed vJoy output) and `runner_check`
- docs/GREMLIN_ACTIONS.md  all Gremlin actions, inputs, JoyGob status and per-action notes; update when adding one
- planned: gob-plugin
- tests/fixtures   sample_r14.xml (ours); local/ = personal profiles, gitignored, tested if present
- ref/             cloned upstream sources (gitignored) - read specific files instead of web search

## Rules
- UI: icons first (egui-phosphor via `style::icon`), text only as tooltips/short captions; helpers in joygob/src/style.rs.
  Layout: left icon nav rail; all options live in settings_view.rs under horizontal tabs.
- All user-visible strings go through i18n keys; add key to lang/en.ftl AND lang/ru.ftl.
- Gremlin import must round-trip: unknown actions kept as raw XML.
- GUI never blocks; engine runs on its own thread, talks via channels.
- Settings: TOML in %APPDATA%\JoystickGobloon\settings.toml.
- Messages: technical output (processing results, import/engine warnings, errors) goes to the log file
  %APPDATA%\JoystickGobloon\joygob.log via the `log` crate (English, not i18n; joygob/src/logfile.rs).
  Only warnings/errors that matter to a general user become dialogs (`JoyGobApp::error` / `alerts`),
  translated; routine info goes to the status bar. No report/diagnostics windows.
- Commits: Conventional Commits (feat/fix/docs/refactor/test/chore/build/ci; scope = crate, e.g. `feat(gob-gremlin): parse R14 library`).

## Testing notes
- Never change the user's real HidHide/vJoy state in tests without asking.
- No synthetic mouse clicks for GUI checks: other windows may overlap and receive them. Screenshot only, ask the user to click.
- HidHide hides the VKB sticks; target\debug\JoystickGobloon.exe and examples\monitor.exe are whitelisted.
- Profile device GUIDs come from another PC: use the substitution banner on the Devices page.
- JoyGob whitelists its own exe in HidHide on start: scratch builds land there too, unregister them afterwards.
- vJoy reports only on change: start `monitor` BEFORE activating a profile to see vJoy output.

## Commands
- cargo build / cargo run -p joygob
- cargo test -p <crate>
- cargo clippy --workspace
