<p align="center">
  <img src="assets/icon/JoystickGobloon.svg" width="140" alt="Joystick Gobloon logo">
</p>

<h1 align="center">Joystick Gobloon</h1>

<p align="center">
  <b>Device hiding, virtual joysticks and input mapping for Windows and Linux — in one app.</b>
</p>

<p align="center">
  <a href="https://github.com/h0useRus/JoystickGobloon/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/h0useRus/JoystickGobloon?label=release"></a>
  <a href="https://github.com/h0useRus/JoystickGobloon/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/h0useRus/JoystickGobloon/ci.yml?branch=dev&label=CI"></a>
  <img alt="Windows 10 / 11" src="https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4">
  <img alt="Linux" src="https://img.shields.io/badge/Linux-AppImage-FCC624">
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-green"></a>
</p>

<p align="center">
  <a href="https://h0userus.github.io/gobloon/"><b>Website</b></a> ·
  <a href="https://github.com/h0useRus/JoystickGobloon/releases/latest"><b>Download</b></a> ·
  <a href="https://h0userus.github.io/gobloon/learn.html"><b>Documentation</b></a> ·
  <a href="https://h0userus.github.io/gobloon/emulation.html"><b>Virtual devices and hiding</b></a>
</p>

<p align="center">
  <img src="https://h0userus.github.io/gobloon/img/vjoy.png" width="860" alt="vJoy page: device tabs, the device editor and the output tester">
</p>

---

HOTAS players usually run three tools side by side: [HidHide](https://github.com/nefarius/HidHide) to hide the
physical sticks from games, [vJoy](https://github.com/jshafer817/vJoy) for virtual joysticks, and
[Joystick Gremlin](https://github.com/WhiteMagic/JoystickGremlin) to map one to the other. **Joystick Gobloon**
replaces all three user interfaces with one fast, native app written in Rust. It talks to the drivers directly,
imports Joystick Gremlin profiles and keeps the workflow you know.

## Features

- **Opens Joystick Gremlin profiles** (R14 and later) without losing anything; **Map controls** matches a profile
  made on another PC to your devices.
- **Every Gremlin action** — map to vJoy, response curves, tempo, double tap, chain, macros, keyboard and mouse
  output, merge and split axis, conditions, sounds, text to speech and more — plus modes, undo / redo.
- **Modifiers and layers**: hold M1, M2 or both and the same button or axis does something else.
- **Virtual joysticks and gamepads** configured right in the app; profiles remember the ones they use.
- **Device hiding** with one click on a stick's eye, so games see only the virtual devices.
- **Input viewer and calibration**, tray icon, light and dark theme, seven languages.

| | Windows 10 / 11 | Linux |
| --- | --- | --- |
| Package | Installer (with the drivers) or portable zip, self-updating | AppImage, self-updating |
| Virtual devices | vJoy joysticks | uinput joysticks and Xbox-style gamepads |
| Device hiding | HidHide, for every program | Exclusive reading while the app runs |

Everything else — every action with a picture of its editor, Windows and Linux differences, the FAQ — is on the
[website](https://h0userus.github.io/gobloon/).

## Getting started

**Windows 10 / 11 (x64):** download the installer from the
[latest release](https://github.com/h0useRus/JoystickGobloon/releases/latest) (or the portable zip). It installs the
signed **HidHide** and **vJoy 2.1.9.1** drivers when they are missing. Import your Joystick Gremlin profile, use
**Map controls** on the Devices page if it was made on another PC, hide your sticks and press **Activate** (▶).

**Linux (x86_64):** download `JoystickGobloon-<version>-x86_64.AppImage`, `chmod +x` it and start it. Click
**Grant access** once on the Devices page, then import your profile as above.

Code signing through SignPath is being set up ([code signing policy](docs/CODE_SIGNING.md)); until then Windows
SmartScreen may warn on the first start: *More info → Run anyway*.

> **Status:** alpha — daily-driver features are in place; expect rough edges.

## Building from source

Rust is pinned in [`rust-toolchain.toml`](rust-toolchain.toml).

```sh
cargo run -p joystick-gobloon        # debug build
cargo test --workspace
```

Translations are [Fluent](https://projectfluent.org/) files in [`lang/`](lang): copy `en.ftl` to `<code>.ftl`,
translate it and put the `lang` folder next to the executable, or open a pull request.

## Credits

Built on [HidHide](https://github.com/nefarius/HidHide) by Benjamin Höglinger-Stelzer (Nefarius),
[vJoy](https://github.com/shauleiz/vJoy) by Shaul Eizikovich (signed 2.1.9.1 build by
[Justin Shafer](https://github.com/jshafer817/vJoy)) and
[Joystick Gremlin](https://github.com/WhiteMagic/JoystickGremlin) by Lionel Ott.
Free code signing by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

## License

[MIT](LICENSE)
