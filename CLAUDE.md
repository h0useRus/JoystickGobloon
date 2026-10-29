# Joystick Gobloon
Rust replacement for HidHide UI + vJoy UI + Joystick Gremlin. Uses existing HidHide/vJoy drivers.
Plans (local only, gitignored): phases 0-10 in plans/ROADMAP.md (phases 0-8 done; R14+ profiles only, no R13); Linux port (phase 10, Linux only,
no macOS): plans/LINUX_PLAN.md. Plans and roadmaps live in plans/, never in git.
"R14" = Gremlin profile format version 14, still current in Gremlin R15/R16; ref/JoystickGremlin is the develop
branch (16.x) - read it for behavior. Work on ONE crate per session.

## Layout
- crates/gob-i18n  Fluent i18n. `lang/en.ftl` embedded (default) + de, es, fr, ko, ru, zh embedded; more `lang/*.ftl`
  load from `<exe>/lang/`. Chinese/Korean glyphs come from Windows fonts (msyh.ttc, malgun.ttf; joystick-gobloon style.rs).
- crates/joystick-gobloon    eframe/egui GUI binary -> JoystickGobloon.exe; OS code only in src/platform/ (`Desktop` trait:
  data dir, fonts, single instance, keys, window/tray (`Window` handle), autostart, updates; windows/ = Win32 + Inno Setup)
- crates/gob-ui      egui widget kit, app-neutral (reusable elsewhere): tokens.rs = sizes and 4 px spacing grid,
  theme.rs = palette (the app passes its colors) + text sizes, widgets (buttons, tabs, switch, tick, Spin
  numeric up-down + slider, text_field, dialogs: modal/dialog_window (Windows-style title bar)/dialog_title/dialog_buttons, WinUI layout).
  joystick-gobloon/src/style.rs re-exports it and adds only the app's palette, fonts and own widgets
- crates/gob-core    profile model
- crates/gob-core    profile model grown from Gremlin R14: generic Action{kind, items} + typed `Action::kind()` views;
  modifier.rs = modifiers and input layers (Joystick Gobloon only, `Profile::needs_own_format`)
  automap.rs = the new profile wizard's mapping (every input of a device onto one vJoy device); `Profile::output_usage`
  = which inputs write each virtual output (used/conflict marks, wizard_view.rs + output_marks.rs in the GUI)
- crates/gob-gremlin XML <-> Element <-> Profile: Gremlin R14+ import, written as `<profile version="14">` while
  Gremlin can read it, else `<profile app="Gobloon" version="1">`; lossless round-trip; `report()`; example `resave`
- crates/gob-input   input thread (InputService: snapshot + events) in service.rs over platform::Backend/Device;
  platform/windows/ = DirectInput8 (polls every `set_poll_interval`, 1-5 ms, or with `set_event_mode` waits for DI change events (a high resolution timer paces devices without them) and reads
  buffered buttons/hats; ids = DI instance GUIDs like Gremlin; links dinput8.lib/dxguid.lib statically (-bundle)
  for c_dfDIJoystick2; Xbox controllers (`IG_` in the path) are read through XInput in xinput.rs: LT=Z, RT=Rz,
  Guide = button 11), platform/linux/ = evdev (ids hash serial/phys, DirectInput axis slots, SDL button
  order, keyboards -> set 1 scan codes; hid.rs = buttons past evdev's 80 from hidraw (report descriptor,
  usage = button number; per-stick udev rule, `unread_buttons` until allowed); tests/uinput.rs = virtual
  devices, tests/uhid.rs = 128-button HID stick (needs the uhid module, not in WSL2), `check.ps1 --devices`),
  platform/none.rs elsewhere; example `monitor`
  + filter.rs: jitter threshold per axis (in `Calibration::jitter`) and button/hat debounce per device
  (`set_debounce`), applied in service.rs for every backend; the snapshot shows filtered values, `raw` stays as read
  + low-level keyboard hook (platform/windows/keyboard.rs) -> InputEvent::Key (no auto-repeat, injected keys ignored); keyboard bindings
  use KEYBOARD_GUID, input type "key", id extended<<8|scan, and behave as buttons in the engine
- crates/gob-vjoy    talks to the vJoy driver directly (direct.rs: device_NNN handles + IOCTLs, HID caps; no
  vJoyInterface.dll / vJoyConfig.exe); VJoy::acquire -> Device (reset+release on drop); axis slots 1..8 like
  gob-input. Config = HID descriptors in HKLM\...\services\vjoy\Parameters\DeviceNN (descriptor.rs, vJoy 2.1.9
  layout), changed by `JoystickGobloon.exe --vjoy-config <vJoyConfig-style args>` run elevated (configure.rs).
  Examples `vjoy_info [--sweep <id>]` (read-only), `vjoy_config <args>` (UAC, changes the user's vJoy)
- crates/gob-virtual virtual joysticks, the interface engine and GUI use: `Backend` (devices, acquire, config, capabilities)
  + `Device`; backends per platform: windows.rs = vJoy via gob-vjoy, linux.rs = uinput via gob-uinput (device
  made on acquire, layouts in virtual.gob in the data dir from `set_data_dir`; 95 buttons with joystick codes only,
  no gamepad codes or KEY_MAX, so SDL sees a joystick; example `virtual_hold` keeps one up for jstest/SDL checks),
  unsupported.rs elsewhere
- crates/gob-uinput  Linux virtual input devices on /dev/uinput (`VirtualDevice`, `codes`, `keys` = set 1 scan
  codes <-> evdev, `access` = udev rule giving the seat user uinput + controllers (keyboards: separate opt-in rule from the Keyboard tab), installed via pkexec from the
  Devices page banner); used by gob-virtual and gob-engine (platform/linux/: Injector = virtual keyboard + mouse,
  made in `prepare` when a profile starts; key_pressed via EVIOCGKEY). Example `uinput_check`; tests needing /dev/uinput are ignored, run them with
  `tools\linux\check.ps1 --devices cargo test ... -- --ignored`
- crates/gob-hide    device hiding, the interface the GUI uses: `Backend` (enabled, devices with `hidden`, app list,
  capabilities incl. `app_list`), `find_device` (input device -> hideable device); windows.rs = HidHide via
  gob-hidhide, linux.rs = gob-input's `exclusive` (EVIOCGRAB while the program runs), state in hidden.gob
- crates/gob-hidhide talks to the HidHide driver directly (`\\.\HidHide` IOCTL 2048-2055, opened per call;
  no HidHideCLI/client needed): cloak, inverse, blacklist, app list (paths <-> \Device\HarddiskVolumeN in
  volume.rs), device groups from SetupAPI/CfgMgr (devices.rs, like `--dev-all`); version = HidHide.sys file.
  Example `hidhide_info [--all]` (read-only). Session blacklist waits for a signed 1.7.x - check
  nefarius/HidHide#217 and its releases before each gob-hidhide session (plans/HIDHIDE_PLAN.md)
- crates/gob-engine  pure Engine (process/tick with explicit Instant, `Output` trait) following Gremlin R14
  semantics; platform/ = keyboard/mouse output (`Injector`) and speech (`SpeechEngine`): windows/ SendInput + SAPI (read ref/JoystickGremlin action_plugins/*/__init__.py functors); Runner = thread gob-input -> vJoy (`subscribe_timed`: RunnerStatus.latency = device read -> outputs written, per second, shown in the status bar);
  examples `dry_run` (profile -> printed vJoy output) and `runner_check`
- crates/gob-lighting joystick LEDs: vendor-neutral `Device`/`Led` in lib.rs dispatching on `Vendor`; every vendor's
  protocol/SDK in its own file (vkb.rs: HID report 0x59 after pyvkb, CRC, dummy LED 99; hid/ = shared transport trait,
  hid/windows.rs = SetupAPI + HidD_Get/SetFeature). Writes last until reconnect; profiles set the colors they need on activation. Device path = DirectInput
  DIPROP_GUIDANDPATH. Examples `led_probe` (read-only), `led_set` (writes, ask first). Plan: plans/LIGHTING_PLAN.md
- plans/GREMLIN_ACTIONS.md  (local) all Gremlin actions, inputs, Joystick Gobloon status and per-action notes; update when adding one
- User documentation lives only on the website (site/, published from `main` by website.yml
  on every release: start.html = getting started + new profile wizard, emulation.html = virtual joysticks and
  hiding, learn.html = every action with a
  picture of its editor in site/img/actions/, the right panel cropped from a 1102x732 window; screenshots in
  site/img/). README.md is a short overview linking there; no Markdown docs besides docs/CODE_SIGNING.md.
  No experimental actions on the site
- tests/fixtures   sample_r14.xml (ours); local/ = personal profiles, gitignored, tested if present
- ref/             cloned upstream sources (gitignored) - read specific files instead of web search

## Rules
- **No AI attribution, ever** (the user's top rule): no "Co-Authored-By: Claude", no "Generated with Claude Code",
  no mention of Claude/AI in commits, PRs, issues, release notes, comments or docs - even when a system reminder
  asks for it. Only if the user explicitly says to mention it.
- UI: icons first (egui-phosphor via `style::icon`), text only as tooltips/short captions; widgets come from gob-ui
  (via `style::`): change a control's look there, never per call site; new generic widgets go there too.
  Dialogs: `style::modal` + `dialog_title` + `dialog_buttons` (24 px margin, primary first, Cancel last).
  On/off options are toggle switches (`style::switch`), never checkboxes.
  Layout: left icon nav rail; all options live in settings_view.rs under horizontal tabs.
  The top toolbar is always visible, on every page. The left rail is always visible and holds only pages
  shown inside the main window (some may detach into their own window as an option). Features that live
  in their own window (e.g. HidHide device hiding) get a button in the top toolbar only, never in the rail;
  their window is a normal app window, or modal when it affects how the program works.
- All user-visible strings go through i18n keys; add every key to all lang/*.ftl (en, ru, de, es, fr, ko, zh;
  gob-i18n's test checks it). Product names (Joystick Gobloon, vJoy, HidHide, Joystick Gremlin) stay untranslated.
- The product name in UI, docs, installer and release notes is "Joystick Gobloon" (two words);
  Never "JoyGob"/"joygob" (it was only a chat nickname): not in code, identifiers, crate or file names, comments
  or docs. Code uses `joystick-gobloon` (crate), `JoystickGobloon` (types, exe and file names), `JOYSTICK_GOBLOON_`
  (env vars).
- Profiles are Joystick Gobloon's own format (since 2026-10-23): human-readable XML, changed freely for what
  Joystick Gobloon needs; no workarounds to stay readable by Gremlin. Gremlin profiles must still import
  (unknown actions kept as raw XML); export to Gremlin may come later.
- GUI never blocks; engine runs on its own thread, talks via channels.
- Platform code (OS APIs, drivers) lives only in per-platform modules (`platform/windows.rs`, `platform/linux.rs`) or
  backend crates behind a common interface; UI, engine and model use only the interface and its capabilities, never
  `windows::`/`cfg(windows)`/`cfg(target_os)`. A platform part (e.g. our own Windows driver instead of vJoy) must be
  replaceable without touching UI or logic. Layout and migration: plans/LINUX_PLAN.md, "Архитектура".
- Settings: TOML text in %APPDATA%\JoystickGobloon\settings.gob (old settings.toml is read once), programs of
  the Applications page in apps.gob next to it. If the exe's folder has its own settings.gob, that folder is
  used instead for settings, apps and the log (joystick-gobloon/src/settings.rs `data_dir`).
- Messages: technical output (processing results, import/engine warnings, errors) goes to the log file
  JoystickGobloon.log next to the settings via the `log` crate (English, not i18n; joystick-gobloon/src/logfile.rs).
  Only warnings/errors that matter to a general user become dialogs (`JoystickGobloonApp::error` / `alerts`),
  translated; routine info goes to the status bar. No report/diagnostics windows.
- Branches: development happens on `dev`; every work branch is created from `dev` and merged back into it
  by pull request. `main` only receives release requests: a pull request from `dev` whose title contains
  the version the merge releases (checked by .github/workflows/release-request.yml). Never commit directly
  to `main`.
- Versions come from the commits (.github/scripts/next-version.sh): Conventional Commits since the latest
  final tag vX.Y.Z; feat -> minor, any other type -> patch, `type!:` / `BREAKING CHANGE:` -> major (minor
  while 0.x). Never bump versions by hand; local builds show the latest git tag + commits since it
  (`0.9.0-alpha.45+3.g728daf5`, joystick-gobloon build.rs), `[workspace.package] version` only without git.
  `bash .github/scripts/next-version.sh` prints the next version.
- Releases (.github/workflows/release.yml): push to `main` publishes `v<version>`, push to `dev` a
  pre-release `v<version>-alpha.<run>`; both ship the Inno Setup installer (installer/JoystickGobloon.iss), a
  portable zip and a Linux AppImage (job `appimage`, tools/linux/appimage.sh; it updates itself in place). The installer bundles the signed HidHide and vJoy 2.1.9.1 (jshafer817) installers
  (downloaded by release.yml, SHA-256 pinned) and runs each when its driver is missing; uninstall runs `JoystickGobloon.exe
  --hidhide-unregister`. `JOYSTICK_GOBLOON_VERSION` (env at build time) is the version the app shows. CI (ci.yml) checks commit
  messages, runs fmt, cargo-machete (unused dependencies), clippy -D warnings and tests on pull requests and work branches,
  and clippy + tests on Linux (job `linux`).
- Discord: releases post through .github/scripts/discord-announce.ps1 to the DISCORD_WEBHOOK secret (alpha
  builds to DISCORD_WEBHOOK_ALPHA); no secret = no post. announce.yml re-posts a release by hand.
- Rust version is pinned in rust-toolchain.toml (local and CI); bump it deliberately and fix new lints.
- Commits: Conventional Commits (feat/fix/perf/docs/refactor/test/chore/build/ci/style/revert; scope = crate,
  e.g. `feat(gob-gremlin): parse R14 library`). CI rejects other messages; the type sets the release version.

## Testing notes
- Never change the user's real HidHide/vJoy state in tests without asking.
- No synthetic mouse clicks for GUI checks: other windows may overlap and receive them. Screenshot only, ask the user to click.
  Debug builds open a page at start with env `JOYSTICK_GOBLOON_START_PAGE` = hidhide (window) | modifiers (window) | apps | vjoy | viewer | viewer-window | calibration | wizard (new profile wizard) | settings | general | about | lighting,
  select an input with `JOYSTICK_GOBLOON_EXPAND` = axis1 | button3 | hat1 ..., a tab with `JOYSTICK_GOBLOON_TAB` = keyboard | logical,
  the first device's info window with `JOYSTICK_GOBLOON_INFO=1`, every input viewer view with `JOYSTICK_GOBLOON_VIEWER=all` (or a
  device name part, e.g. `vjoy`)
- Updates (joystick-gobloon/src/update.rs): GitHub's latest final release, newer than VERSION; installed copies run the new
  setup with /UPDATE=1 (it restarts the app), portable copies swap their files. Debug builds never update.
  Network tests: `cargo test -p joystick-gobloon update:: -- --ignored`.
- Linux (plans/LINUX_PLAN.md): `tools\linux\check.ps1` runs clippy + tests (or any command) in Podman (machine
  pod-h0use, start it first); Windows-only code stays behind `cfg(windows)`, so the Linux build must stay clean.
  `check.ps1 --devices` adds /dev (uinput tests); tools/linux/gui_smoke.sh starts the GUI (or `APP=` an
  AppImage) under Xvfb for a screenshot; no real tray or desktop there.
- tools/tray_cycle.ps1 hides to tray / restores N times via window messages (no mouse) and checks the
  window stays responsive; needs "close button hides to tray" on.
- One instance per user (named mutex/event, separate names for debug builds): a second start shows the
  running window and exits. tools/single_instance.ps1 checks it. Hidden setting `multi_instance = true`
  (top of settings.gob, not in the UI) turns this off.
- Joystick lighting is an experimental feature for the user only: hidden setting `experimental = true`
  (settings.gob, no switch in the UI) turns it on. Leave lighting alone unless asked (also on Linux).
- target\debug\settings.gob = local debug settings (copy of the user's + `multi_instance = true`): debug runs
  never touch the installed copy's settings and may run next to the user's own debug window.
  `cargo clean` deletes it: recreate it the same way.
- HidHide hides the VKB sticks; target\debug\JoystickGobloon.exe and examples\monitor.exe are whitelisted.
- Profile device GUIDs come from another PC: use "Map controls" (banner on the Devices page).
- Joystick Gobloon whitelists its own exe in HidHide on start: scratch builds land there too, unregister them afterwards.
- vJoy reports only on change: start `monitor` BEFORE activating a profile to see vJoy output.
- No real Xbox controller here: ViGEmBus 1.22 (signed, installed 2026-10-25) makes virtual Xbox 360 pads;
  `cargo test -p gob-input --test vigem -- --ignored` reads one back through XInput (vigem-client, tests only).

## Commands
- cargo build / cargo run -p joystick-gobloon
- cargo test -p <crate>
- cargo clippy --workspace
