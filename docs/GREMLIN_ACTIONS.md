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
| `map-to-keyboard` | B K | done |
| `chain` | B K | done |
| `double-tap` | B K | done |
| `smart-toggle` | B K | done |
| `hat-buttons` | H | done |
| `split-axis` | A | done |
| `merge-axis` | A B | done |
| `axis-delta` | A | done |
| `dual-axis-deadzone` | A | done |
| `condition` | A B H K | done (logical devices are false) |
| `reference` | A B H K | done (editor only: link / copy existing action) |
| `pause-resume` | B K | done |
| `load-profile` | B K | done |
| `play-sound` | B K | done |
| `text-to-speech` | B | done |
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
- Editor: repeat mode/count/delay and the steps (key, pause, mouse button/motion, vJoy button/axis/hat),
  reorder and remove. Each change rewrites only that `<macro-action>`; `joystick` and `logical-device`
  steps are shown as not editable and kept. Exclusive/preemptive flags are kept but not shown.

## map-to-keyboard

- One `<input>` group per key with `scan-code` int and `is-extended` bool.
- Press sends key-down for each key in order, release sends key-up in reverse order (Gremlin queues two
  macros with zero pauses, so no default delay).
- Editor: keys are recorded by polling Windows key states (GetAsyncKeyState + MapVirtualKey), so left/right
  modifiers and the E0 flag are exact; every key pressed until all are released is recorded, in press
  order. Names come from GetKeyNameText (current layout). Keyboard conditions use the same recorder.
- JoyGob addition: if the input is released in a mode where it is bound differently, the keys are still
  released (Gremlin would leave them stuck). Each press releases at most once.

## chain

- Groups `chain-0`, `chain-1`, ... with `<action-id>` lists (ordered by number), `timeout` float (s, 0 = off).
- Editor adds/removes steps and renumbers them from 0 like Gremlin writes them. Note: Gremlin only reads
  steps that contain an action, so an empty step disappears when Gremlin loads the profile.
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

## axis-delta

- `threshold` float (default 0.1), `positive-actions`, `negative-actions`. Activation disabled.
- Uses the raw axis value (not one changed by earlier actions). The first event only sets the reference;
  an exact 0.0 is skipped without updating it (Gremlin's Python falsiness). Changes accumulate; reaching
  +threshold pulses the positive list, -threshold the negative list (press, release 50 ms later), then the
  accumulator resets. Gremlin sleeps 50 ms in the event thread; JoyGob uses a timer.

## dual-axis-deadzone

- Properties: `label`, `inner-deadzone` (default 0.0), `outer-deadzone` (default 1.0), `axis1-guid`/`axis1-axis`,
  `axis2-guid`/`axis2-axis`; children `output1-actions`, `output2-actions`. Activation disabled.
- On any event reads both axes' latest values (x, y). With alpha = atan2(y, x), the inner limits are
  |cos(alpha)|*inner and |sin(alpha)|*inner (a circle), the outer limit is `outer` on both (a square);
  each axis becomes clamp((|v| - low) / (outer - low), 0, 1) with v's sign and goes to its own list.
  Coinciding limits (Gremlin's ZeroDivisionError) send nothing.
- Like merge-axis it is usually bound to both axes; the add menu offers to reuse it.

## reference

- Not a runtime action: in Gremlin's editor it is a placeholder that is replaced by an existing library
  action, either the same one (shared) or a deep copy, and it writes nothing to the profile.
- JoyGob: the add menu has "Link an existing action" and "Copy an existing action". Roots and actions that
  would create a loop (the list's own action or one containing it) are not offered; `Profile::add_ref`
  refuses loops too. A copy duplicates the whole subtree with new ids, keeping sharing inside it.

## condition

- `logical-operator` `all`/`any`; `<condition>` groups with `condition-type` (`current_input`, `joystick`,
  `keyboard`, `vjoy`, `logical_device`) and a `<comparator>` (`range`: `lower-limit`/`upper-limit`;
  `pressed`: `is-pressed`; `direction`: `direction` hat names). Joystick: `<input>` groups with
  `device-guid`, `input-type`, `input-id`; keyboard: `<input>` with `scan-code`, `is-extended`; vJoy:
  `vjoy-id`, `input-type`, `input-id` on the condition. Children `true-actions`, `false-actions`.
  Activation both (activation-mode honoured).
- Comparators: range uses the first state (bools count as 0/1), pressed needs every state equal
  (Python's `1.0 == True`), direction the first state in the list. No conditions: all -> true, any -> false.
- Button: the branch chosen on press also gets the release. Hat: center goes to the cached branch; a
  direction that switches branch first sends center to the old one. Axis: evaluated every event.
- Joystick states are JoyGob's latest physical values (the runner seeds buttons/hats/axes at start),
  device GUIDs follow the substitution; vJoy states are the last values JoyGob wrote; keys via
  GetAsyncKeyState.

## load-profile, play-sound, text-to-speech

- load-profile: `load-profile` string (absolute path), activation press. Runs while paused too (Gremlin's
  AlwaysExecute). The engine asks the GUI (`Output::load_profile` -> `RunnerStatus::load_profile`), which
  loads the file (asking to save unsaved edits first) and activates it.
- play-sound: `filename`, `volume` int 0..100 (default 50), activation press. Played one after another
  (Gremlin's default "Sequential" mode) with rodio: wav, mp3, ogg vorbis, flac.
- text-to-speech: `text`, `queue-mode` (`queue-back`, `queue-front`, `interrupt`), `playback-rate` and
  `playback-pitch` -1..1, `playback-volume` 0..1 (Qt ranges), activation press. `$current_mode` /
  `${current_mode}` become the active mode. Spoken by Windows SAPI with per-request XML (rate and pitch
  scaled to -10..10, volume to 0..100); `interrupt` purges, `queue-front` cannot reorder SAPI's queue and
  behaves like `queue-back`.
- JoyGob addition: `voice` string property (installed voice name, classic SAPI and OneCore voices); Gremlin
  ignores it on load and drops it when it saves. Missing voice -> system default. The editor previews
  sounds and speech (speech interrupts the previous preview).
