language-name = English

# Navigation and toolbar tooltips
nav-devices = Devices and bindings
nav-hidhide = Device hiding
nav-apps = Applications
nav-vjoy = Virtual joysticks
nav-viewer = Input viewer
nav-calibration = Calibration
nav-settings = Settings
action-open = Import Joystick Gremlin profile
action-save = Save
label-mode = Mode

# Settings
settings-appearance = Appearance
settings-general = General
settings-about = About
settings-language = Language
settings-reopen-last = Reopen the last profile on startup
theme-light = Light
theme-dark = Dark
theme-system = System
theme-system-tip = Follow the Windows light or dark setting
about-text = Device hiding, virtual joysticks and input mapping in one app.

# Empty states
no-profile = No profile loaded
device-linked = Bindings of the profile device with the same name. Use “Map controls” to save the link.
device-disconnected = Not connected
no-devices-attached = No game controllers found

# Status bar
status-ready = Ready
status-loaded = Loaded { $file }
status-saved = Saved { $file }
import-error = Could not open the profile: { $error }
save-error = Could not save the profile: { $error }
file-filter-gremlin = Joystick Gremlin profile

# Import report
alert-error = Something went wrong
alert-import = Some actions in { $file } will not work
alert-open-log = Open the log
ok = OK
hidhide-whitelist-error = Could not add Joystick Gobloon to the HidHide whitelist: { $error }
warn-not-executable = “{ $kind }” × { $count }: kept in the profile, but not executed yet
warn-scripts = The profile contains Python plugins: they are kept, but not run

# Map controls (device substitution)
subst-title = Map controls
subst-auto = Auto
subst-auto-tip = Map the remaining devices to attached ones with similar names; unclear ones stay for you to choose
subst-banner = Device IDs in the profile do not match this PC: { $count }
subst-keep = Keep as is
subst-apply = Apply
status-substituted = Controls mapped. Save the profile to keep the change.

# vJoy
vjoy-missing = vJoy is not installed or could not be loaded.
vjoy-enabled = Driver enabled
vjoy-disabled = Driver disabled
vjoy-no-devices = No vJoy devices configured yet.
vjoy-axes = Axes
vjoy-buttons = Buttons
vjoy-hats = Hats
vjoy-test-hint = Test output: lock the device to use the controls below.
vjoy-lock = Take exclusive control of this device to test its output
vjoy-unlock = Release the device
vjoy-free = Free
vjoy-own = Controlled by Joystick Gobloon
vjoy-busy = Used by another program (e.g. Joystick Gremlin)
vjoy-unknown = Unknown state
vjoy-error = Could not take the device: { $error }

# Device hiding
hidhide-missing = HidHide is not installed.
hidhide-on = Device hiding is on
hidhide-off = Device hiding is off
hidhide-hidden = Hidden: only whitelisted applications see it
hidhide-visible = Visible to every application
hidhide-filter-disconnected = Hide disconnected devices
hidhide-gaming-only = Game controllers only
hidhide-enable = Enable device hiding
hidhide-hint = Click the eye to hide a device from other programs (closed eye). Reconnect it afterwards for the change to take effect.
hidhide-no-devices = No devices to show.
action-refresh = Refresh
settings-hidhide-self = Add Joystick Gobloon to the HidHide whitelist on startup
status-whitelisted = Joystick Gobloon was added to the HidHide whitelist.

# Applications
apps-add = Add application
apps-empty = No applications yet. Add a game or a program to set it up.
apps-missing = File not found
apps-missing-text = The program is not there any more. Remove it from the list?
apps-sees = Sees hidden devices
apps-blind = Does not see hidden devices
apps-inverse = Inverse application cloak
apps-inverse-tip = Listed programs do not see hidden devices; all other programs do
apps-remove = Remove from the list

# Engine
action-activate = Activate the profile
action-deactivate = Stop the profile
title-active = Active
title-inactive = Inactive
status-active = Profile active
status-stopped = Profile stopped
activate-error = Could not activate the profile: { $error }

# Editing
edit-add = Add action
edit-remove = Remove
edit-up = Move up
edit-down = Move down
edit-not-editable = This action is kept in the profile but cannot be edited yet.
edit-on-press = On press
edit-on-release = On release
edit-on-both = On press and release
edit-inverted = Inverted
edit-threshold = Long press threshold
edit-short = Short press
edit-long = Long press
edit-deadzone = Deadzone
edit-invert = Invert
edit-inherited = Inherited from mode “{ $mode }”
edit-unbind = Remove all actions of this input
edit-override = Override in this mode
input-axis-kind = Axis
input-button-kind = Button
input-hat-kind = Hat
kind-map-to-vjoy = Map to vJoy
kind-response-curve = Response curve
kind-change-mode = Change mode
kind-tempo = Short / long press
kind-description = Description

# Files and modes
action-new = New profile
action-save-as = Save as…
status-new = New profile
unsaved-title = Unsaved changes
unsaved-text = Save changes to { $file }?
unsaved-save = Save
unsaved-discard = Don't save
cancel = Cancel
modes-edit = Edit modes
modes-title = Modes
modes-name = Name
modes-parent = Inherits from
modes-remove = Remove mode and its bindings
modes-new = New mode
modes-add = Add mode

# Undo
action-undo = Undo (Ctrl+Z)
action-redo = Redo (Ctrl+Y)
status-undone = Undone
status-redone = Redone
edit-curve-help = Drag a point or handle to move it · Click empty space to invert · Double-click to add a point · Right-click a point to remove it

# vJoy mapping
vjoy-device-n = vJoy #{ $n }
vjoy-axis-1 = X Axis
vjoy-axis-2 = Y Axis
vjoy-axis-3 = Z Axis
vjoy-axis-4 = X Rotation
vjoy-axis-5 = Y Rotation
vjoy-axis-6 = Z Rotation
vjoy-axis-7 = Slider
vjoy-axis-8 = Dial
vjoy-button-n = Button { $n }
vjoy-hat-n = Hat { $n }
vjoy-absolute = Absolute
vjoy-relative = Relative
vjoy-speed = Speed

# Mouse
kind-map-to-mouse = Map to mouse
mouse-mode-button = Mouse button
mouse-mode-motion = Cursor motion
mouse-left = Left button
mouse-right = Right button
mouse-middle = Middle button
mouse-forward = Forward button
mouse-back = Back button
mouse-wheel-up = Wheel up
mouse-wheel-down = Wheel down
mouse-direction = Direction
mouse-min-speed = Minimum speed
mouse-max-speed = Maximum speed
mouse-time-to-max = Time to maximum speed

# Toggle, split, pause, run
kind-smart-toggle = Smart toggle
kind-split-axis = Split axis
kind-pause-resume = Pause / resume
kind-run-command = Run command
edit-delay = Hold time for momentary
edit-split = Split at
edit-lower = Lower part
edit-upper = Upper part
pause-op-pause = Pause
pause-op-resume = Resume
pause-op-toggle = Toggle
run-executable = Program
run-arguments = Arguments
run-browse = Choose program
status-paused = Profile paused

# Double tap, chain
kind-double-tap = Double tap
kind-chain = Chain
edit-tap-threshold = Time to the second tap
edit-tap-exclusive = Exclusive: a single tap waits to rule out a double tap
edit-tap-combined = Combined: a double tap also runs the single tap actions
edit-single = Single tap
edit-double = Double tap
edit-chain-timeout = Restart from the first step after this idle time (0 = never)
edit-chain-step = Step { $n }

# Hat as buttons
kind-hat-buttons = Hat as buttons
edit-hat-four = 4 directions
edit-hat-eight = 8 directions (with diagonals)
hat-north = North
hat-north-east = North-east
hat-east = East
hat-south-east = South-east
hat-south = South
hat-south-west = South-west
hat-west = West
hat-north-west = North-west
hat-center = Center

# Merge axis
kind-merge-axis = Merge axis
edit-reuse = Add this existing action here too (shared, like Gremlin binds it to both axes)
edit-merge-name = Name
edit-merge-no-device = (no device)
edit-merged = Merged axis
merge-average = Average
merge-minimum = Minimum
merge-maximum = Maximum
merge-sum = Sum
merge-bidirectional = Bidirectional (axis 2 minus axis 1)
merge-maximum-deflection = Maximum deflection
merge-closest-to-center = Closest to center

# Axis delta
kind-axis-delta = Axis delta
edit-delta-threshold = Axis change that triggers a pulse
edit-delta-positive = Axis increased
edit-delta-negative = Axis decreased

# Dual axis deadzone
kind-dual-axis-deadzone = Dual axis deadzone
edit-dual-inner = Inner deadzone (circle around the center)
edit-dual-outer = Outer limit (full deflection from here)
edit-dual-close = Inner and outer deadzone are too close to each other.
edit-dual-first = First axis output
edit-dual-second = Second axis output

# Existing actions (Gremlin's reference action)
edit-link-existing = Link an existing action
edit-copy-existing = Copy an existing action

# Condition
kind-condition = Condition
edit-cond-all = All conditions must hold
edit-cond-any = Any condition is enough
edit-cond-add = Condition
edit-cond-true = If true
edit-cond-false = Otherwise
cond-current = This input
cond-joystick = Joystick input
cond-vjoy = vJoy output
cond-keyboard = Keys held
cond-range = Axis between
cond-pressed = Pressed
cond-released = Released

# Sound, speech, load profile
kind-play-sound = Play sound
kind-text-to-speech = Text to speech
kind-load-profile = Load profile
file-filter-sound = Sound file
edit-volume = Volume
edit-file-missing = File not found
edit-tts-hint = Text to speak; $current_mode is replaced with the active mode
edit-tts-rate = Speed
edit-tts-pitch = Pitch
edit-tts-back = Queue after everything else
edit-tts-front = Queue next (Windows speech: same as after everything else)
edit-tts-interrupt = Interrupt what is being said
edit-preview = Preview
edit-tts-voice = Voice
edit-tts-default-voice = Default voice
edit-tts-voice-missing = not installed
kind-map-to-keyboard = Map to keyboard

# Key recording
keys-record = Record keys: press them together, recording ends when all are released
keys-press = Press the keys…
keys-none = No keys
keys-cancel = Cancel recording
edit-chain-add = Add step
edit-chain-remove = Remove this step and its actions
kind-macro = Macro

# Macro editor
macro-repeat = Repeat
macro-repeat-single = Once
macro-repeat-count = Number of times
macro-repeat-hold = While held
macro-repeat-toggle = Until pressed again
macro-repeat-count-tip = How many times
macro-repeat-delay = Pause between repetitions
macro-add-step = Step
macro-step-key = Key
macro-step-pause = Pause
macro-step-mouse-button = Mouse button
macro-step-mouse-motion = Mouse motion
macro-step-vjoy-button = vJoy button
macro-step-vjoy-axis = vJoy axis
macro-step-vjoy-hat = vJoy hat
macro-press = Press
macro-release = Release

# Logical device
logical-device = Logical device
logical-device-tip = Virtual inputs of this profile: map inputs to them, then bind them like a stick
logical-add-axis = Add an axis
logical-add-button = Add a button
logical-add-hat = Add a hat
logical-remove = Remove this input and its bindings
logical-empty = No logical inputs yet: add one with the buttons above.
kind-map-to-logical-device = Map to logical device
cond-logical = Logical device input

# Tray and startup
settings-tray = Show the tray icon
settings-close-to-tray = Close to the tray
settings-minimize-to-tray = Minimize to the tray
settings-tray-title = Tray icon
settings-autostart = Start with Windows (minimized)
settings-activate-on-start = Activate the profile on startup
tray-show = Show Joystick Gobloon
tray-hiding = Device hiding
tray-quit = Quit
autostart-error = Could not change starting with Windows: { $error }

# About
about-alpha = alpha build
about-created = Created by { $name }
about-support = Support development
about-support-tip = Contribute via PayPal
about-source = Source code
about-issues = Report a problem
about-releases = Releases
about-credits = Uses the HidHide and vJoy drivers; profiles are compatible with Joystick Gremlin.
about-license = Free and open source under the MIT license.

# Input viewer
viewer-history = Axes over time
viewer-current = Axes now
viewer-buttons = Buttons and hats
viewer-axis = Axis { $n }

# vJoy configuration
vjoy-add = Add a vJoy device
vjoy-new-device = New vJoy #{ $n }
vjoy-enable = Enable the vJoy driver
vjoy-disable = Disable the vJoy driver
vjoy-hat-continuous = Continuous
vjoy-hat-discrete = 4 directions
vjoy-ffb = Force feedback
vjoy-ffb-enable = Enabled
vjoy-ffb-const = Constant
vjoy-ffb-ramp = Ramp
vjoy-ffb-sq = Square
vjoy-ffb-sine = Sine
vjoy-ffb-tr = Triangle
vjoy-ffb-stup = Sawtooth up
vjoy-ffb-stdn = Sawtooth down
vjoy-ffb-spr = Spring
vjoy-ffb-dm = Damper
vjoy-ffb-inr = Inertia
vjoy-ffb-fric = Friction
vjoy-config-create = Create
vjoy-config-apply = Apply
vjoy-config-revert = Revert
vjoy-config-remove = Remove device
vjoy-config-admin = Windows asks for administrator rights to change vJoy
vjoy-config-locked = Stop the profile to change vJoy devices.
vjoy-config-reading = Reading the vJoy configuration...
vjoy-config-label = configure vJoy #{ $n }
vjoy-config-done = vJoy configuration changed.
vjoy-config-error = Could not change the vJoy configuration: { $error }
vjoy-config-reboot = vJoy configuration changed. Restart Windows for it to take effect.
vjoy-remove-title = Remove vJoy #{ $n }?
vjoy-remove-text = Games and profiles that use this device lose it until it is created again.

# Response curve
curve-type = Curve type
curve-linear = Piecewise linear
curve-cubic = Cubic spline
curve-bezier = Cubic Bezier spline
curve-symmetric = Symmetric
curve-symmetric-tip = Mirror the curve through the center while editing
curve-point = Point
curve-handle = Handle

# Keyboard device
keyboard-device = Keyboard
keyboard-tip = Keyboard keys as inputs: bind actions to keys like to joystick buttons
keyboard-add = Bind a key
keyboard-empty = No keys bound yet: record one with the button above.
edit-binding-description = Description of this input
edit-unbound = No actions yet: add one.
viewer-detach = Open in a separate window
viewer-attach = Back into the main window
viewer-in-window = The input viewer is open in its own window.

# Calibration
calibration-device = Device to calibrate
calibration-raw = Raw
calibration-with-center = With center
calibration-reset = Reset
calibration-save = Save this calibration
calibration-center = Calibrate center
calibration-center-tip = Let the stick rest, wiggle it slightly around the center, then click again
calibration-extrema = Calibrate extrema
calibration-extrema-tip = Move the axis to both ends, then click again
calibration-in-window = The calibration is open in its own window.

# Device info
device-info = Device information
device-info-name = Device name
device-info-axes = Axes
device-info-buttons = Buttons
device-info-hats = Hats
device-info-vid = VID
device-info-pid = PID
device-info-joystick-id = Joystick ID
device-info-guid = Device GUID
device-info-copy = Copy to clipboard
