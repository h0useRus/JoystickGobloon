# Joystick Gremlin R14 actions

Reference for implementing actions in JoyGob. Source: `ref/JoystickGremlin/action_plugins/<name>/__init__.py`
(read the `*Data._to_xml`, defaults in `*Data.__init__` and `*Functor.__call__`).

Input types: A = axis, B = button, H = hat, K = keyboard.

| Action (XML `type`) | Inputs | JoyGob |
|---|---|---|
| `root` | A B H K | done |
| `map-to-vjoy` | A B H K | done (absolute + relative axis) |
| `response-curve` | A | done (piecewise linear only; Gremlin also has cubic spline / bezier, symmetric) |
| `change-mode` | B K | done |
| `tempo` | B K | done |
| `description` | A B H K | done |
| `map-to-mouse` | A B H K | done |
| `macro` | B K | todo (keys, mouse, vJoy, pauses; repeat modes) |
| `map-to-keyboard` | B K | todo |
| `chain` | B K | todo |
| `double-tap` | B K | todo |
| `smart-toggle` | B K | todo |
| `hat-buttons` | H | todo |
| `split-axis` | A | todo |
| `merge-axis` | A B | todo |
| `axis-delta` | A | todo |
| `dual-axis-deadzone` | A | todo |
| `condition` | A B H K | todo |
| `reference` | A B H K | todo (reuses another library action) |
| `pause-resume` | B K | todo |
| `load-profile` | B K | todo |
| `play-sound` | B K | todo |
| `text-to-speech` | B | todo |
| `run-command` | B K | todo |
| `map-to-logical-device` | A B H K | todo (needs logical devices) |

## map-to-mouse

- Properties: `mode` (string `Button`/`Motion`); Button: `button` (`Left`, `Right`, `Middle`, `Forward`,
  `Back`, `Wheel Up`, `Wheel Down`); Motion: `direction` int (compass degrees, 0 = up, 90 = right),
  `min-speed` int, `max-speed` int (px/s), `time-to-max-speed` float (s). Defaults: Left / 0 / 50 / 250 / 1.0.
  Activation `both`. Axis and hat inputs force Motion.
- Axis: constant velocity `min + |v|*(max-min)` with the sign of v, zero below |v| < 0.001; direction
  0 maps to 180 (positive axis moves down), 90 stays 90.
- Button: accelerates from min to max over time-to-max-speed while held. Hat: same, along the hat direction.
- All contributions are summed every 10 ms (100 Hz), sub-pixel residual carried, at most 5 ticks integrated
  after a stall. Motion stops on a mode change unless the input keeps the same binding.
- Wheel Up/Down send one notch (+/-120) on press. Back = XBUTTON1, Forward = XBUTTON2.
