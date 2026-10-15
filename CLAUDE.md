# Joystick Gobloon (JoyGob)
Rust replacement for HidHide UI + vJoy UI + Joystick Gremlin. Uses existing HidHide/vJoy drivers.
Plan: phases 0-10 in docs/ROADMAP.md (phase 0 done). Work on ONE crate per session.

## Layout
- crates/gob-i18n  Fluent i18n. `lang/en.ftl` embedded (default), other `lang/*.ftl` embedded or loaded from `<exe>/lang/`.
- crates/joygob    eframe/egui GUI binary -> JoystickGobloon.exe
- planned: gob-core (profile model), gob-gremlin (Gremlin R14 XML), gob-input (DirectInput8),
  gob-vjoy (vJoyInterface.dll FFI), gob-hidhide (\.\HidHide IOCTL), gob-engine, gob-plugin
- tests/fixtures   real Gremlin profiles
- ref/             cloned upstream sources (gitignored) - read specific files instead of web search

## Rules
- All user-visible strings go through i18n keys; add key to lang/en.ftl AND lang/ru.ftl.
- Gremlin import must round-trip: unknown actions kept as raw XML.
- GUI never blocks; engine runs on its own thread, talks via channels.
- Settings: TOML in %APPDATA%\JoystickGobloon\settings.toml.

## Commands
- cargo build / cargo run -p joygob
- cargo test -p <crate>
- cargo clippy --workspace
