# Joystick Gobloon (JoyGob)
Rust replacement for HidHide UI + vJoy UI + Joystick Gremlin. Uses existing HidHide/vJoy drivers.
Plan: phases 0-10 in docs/ROADMAP.md (phases 0-2 done). Work on ONE crate per session.

## Layout
- crates/gob-i18n  Fluent i18n. `lang/en.ftl` embedded (default), other `lang/*.ftl` embedded or loaded from `<exe>/lang/`.
- crates/joygob    eframe/egui GUI binary -> JoystickGobloon.exe
- crates/gob-core    profile model mirroring Gremlin R14: generic Action{kind, items} + typed `Action::kind()` views
- crates/gob-gremlin XML <-> Element <-> Profile, lossless round-trip; `report()` import warnings; example `resave`
- crates/gob-input   DirectInput8 polling thread (InputService: snapshot + events); ids = DI instance GUIDs like Gremlin;
  links dinput8.lib/dxguid.lib statically (-bundle) for c_dfDIJoystick2; example `monitor`
- planned: gob-vjoy (vJoyInterface.dll FFI), gob-hidhide (\\.\HidHide IOCTL), gob-engine, gob-plugin
- tests/fixtures   sample_r14.xml (ours); local/ = personal profiles, gitignored, tested if present
- ref/             cloned upstream sources (gitignored) - read specific files instead of web search

## Rules
- UI: icons first (egui-phosphor via `style::icon`), text only as tooltips/short captions; helpers in joygob/src/style.rs.
  Layout: left icon nav rail; all options live in settings_view.rs under horizontal tabs.
- All user-visible strings go through i18n keys; add key to lang/en.ftl AND lang/ru.ftl.
- Gremlin import must round-trip: unknown actions kept as raw XML.
- GUI never blocks; engine runs on its own thread, talks via channels.
- Settings: TOML in %APPDATA%\JoystickGobloon\settings.toml.
- Commits: Conventional Commits (feat/fix/docs/refactor/test/chore/build/ci; scope = crate, e.g. `feat(gob-gremlin): parse R14 library`).

## Commands
- cargo build / cargo run -p joygob
- cargo test -p <crate>
- cargo clippy --workspace
