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
| `macro` | B K | done (keys, mouse, vJoy, pauses, repeat modes; not exclusive/preemptive, joystick, logical-device) |
| `map-to-keyboard` | B K | done (engine; editor todo) |
| `chain` | B K | done (editor: timeout + existing steps; adding steps todo) |
| `double-tap` | B K | done |
| `smart-toggle` | B K | done |
| `hat-buttons` | H | done |
| `split-axis` | A | done |
| `merge-axis` | A B | done |
| `axis-delta` | A | todo |
| `dual-axis-deadzone` | A | todo |
| `condition` | A B H K | todo |
| `reference` | A B H K | todo (reuses another library action) |
| `pause-resume` | B K | done |
| `load-profile` | B K | todo |
| `play-sound` | B K | todo |
| `text-to-speech` | B | todo |
| `run-command` | B K | done |
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

## macro

- Properties: `is-exclusive`, `is-preemptive` (bool), `repeat-mode` (`Single`, `Count`, `Hold`, `Toggle`,
  case-insensitive), `repeat-count` int, `repeat-delay` float (s). Steps are `<macro-action type=...>` groups:
  `key` (`scan-code`, `is-extended`, `is-pressed`), `mouse-button` (`button`, `is-pressed`), `mouse-motion`
  (`dx`, `dy`), `pause` (`duration`), `vjoy` (`vjoy-id`, `input-type`, `input-id`, `value` bool / float /
  hat name like `north-east`, axis `axis-mode` absolute/relative), `joystick`, `logical-device`.
- Between two consecutive non-pause steps the profile's `<macro-default-delay>` (default 0.05 s) is inserted.
- Runs when `activation-mode` allows; inside a tempo with `activate-on=release` and the macro on `release`,
  a long press fires it when the button is let go.
- An activation while the same macro runs is queued and started after it (Toggle: stops it instead).
  Count runs N rounds with `repeat-delay` after each; Hold repeats until the input is released; Toggle until
  the next activation. Termination is checked between rounds, so the current round always completes.
- Wheel Up/Down steps scroll one notch regardless of `is-pressed`. Keys go through SendInput with
  virtual key + scan code like Gremlin's `keybd_event`.
- vJoy devices used only by macros are acquired too.

## map-to-keyboard

- One `<input>` group per key with `scan-code` int and `is-extended` bool.
- Press sends key-down for each key in order, release sends key-up in reverse order (Gremlin queues two
  macros with zero pauses, so no default delay).
- JoyGob addition: if the input is released in a mode where it is bound differently, the keys are still
  released (Gremlin would leave them stuck). Each press releases at most once.

## chain

- Groups `chain-0`, `chain-1`, ... with `<action-id>` lists (ordered by number), `timeout` float (s, 0 = off).
- Each event runs the current step's actions; a release advances to the next step (wraps). With a timeout,
  an event more than `timeout` after the previous one (press or release) restarts from step 0.

## double-tap

- `single-actions`, `double-actions`, `threshold` float (s, default 0.5), `activate-on` `exclusive`/`combined`.
- Exclusive: a press starts the timer; released and not pressed again in time -> single pulse at the timeout;
  still held at the timeout -> single press now, single release with the button; second press in time ->
  double actions only.
- Combined: single actions follow the button at once; a second press in time adds the double actions
  (press and release) on top.
- Invalid transitions reset the FSM and send the event to both lists, like tempo.

## hat-buttons

- `button-count` int (4 or 8; switching keeps shared directions, drops diagonal actions); groups named after the directions (`North`, `North-East`, ... `North-West`)
  plus `Center`, each with `<action-id>` lists. Activation disabled.
- Every direction is a virtual button pressed while the hat points exactly there; with 4 buttons a diagonal
  presses none of them. Centered presses `Center`. Children get button values (press, release).
- JoyGob addition: if the hat is centered in a mode where it is bound differently, what the directions
  pressed (vJoy buttons, keys) is still released.

## merge-axis

- Properties: `label` string (its own name, shown when reusing), `axis1-guid`/`axis2-guid` uuid,
  `axis1-axis`/`axis2-axis` int, `operation` (`average`, `minimum`, `maximum`, `sum` (clamped),
  `bidirectional` = (axis2 - axis1) / 2, `maximum-deflection`, `closest-to-center`; ties go to axis 2),
  `actions` children. Activation disabled.
- Gremlin reuses one action on both axes' bindings; JoyGob's add menu offers existing merge-axis actions
  (shared reference) next to a new one, which starts with the edited axis in both slots.
- On any event it reads the latest values of both axes (0 until seen; the runner feeds the initial
  positions), sets the value and runs the children. Axis devices go through the device substitution.
