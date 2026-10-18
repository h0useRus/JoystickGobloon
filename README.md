<p align="center">
  <img src="assets/icon/joygob.svg" width="160" alt="Joystick Gobloon logo">
</p>

<h1 align="center">Joystick Gobloon</h1>

<p align="center">
  Device hiding, virtual joysticks and input mapping for Windows — in one app.
</p>

---

**Joystick Gobloon** is a Windows utility written in Rust for flight-sim and space-sim players
who use HOTAS, joysticks and other game controllers. Today this usually needs three separate tools:

- [HidHide](https://github.com/nefarius/HidHide) hides physical devices from games;
- [vJoy](https://github.com/shauleiz/vJoy) provides virtual joysticks;
- [Joystick Gremlin](https://github.com/WhiteMagic/JoystickGremlin) maps physical inputs to virtual ones with modes, response curves and macros.

Joystick Gobloon brings all three together in a single modern interface. It keeps the familiar
Joystick Gremlin workflow, so you can switch without relearning anything.

> **Status:** early development. Not usable yet. See the [roadmap](docs/ROADMAP.md).

## Planned features

- Import Joystick Gremlin profiles, then save them back in Gremlin's format with nothing lost
- Live view of every axis, button and hat on your devices
- Hide and unhide devices, and manage which applications may see them (uses the HidHide driver)
- Configure vJoy devices and send inputs to them (uses the vJoy driver)
- Modes, response curves, tempo, macros and the other Gremlin actions
- An API for plugins
- English interface by default; more languages can be added as Fluent files (`lang/*.ftl`)
- Light and dark themes

## Requirements

- Windows 10 or 11
- The [HidHide](https://github.com/nefarius/HidHide/releases) and [vJoy 2.1.9.1](https://github.com/jshafer817/vJoy/releases) drivers (the installer installs them when they are missing)

## Building

```sh
cargo run -p joygob            # debug build
cargo build -p joygob --release
```

This produces `target/release/JoystickGobloon.exe`.

## Translations

The English text is built into the program. To add a language, copy [`lang/en.ftl`](lang/en.ftl)
to `lang/<code>.ftl`, translate the values, and put the `lang` folder next to the executable.

## License

[MIT](LICENSE)
