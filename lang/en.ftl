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

settings-lighting = Lighting
settings-leds-hint = When a profile that changed a joystick's LEDs is deactivated, these colors are set. The joystick keeps changed colors until it is reconnected, so its own LED settings cannot be brought back.
settings-leds-none = No joystick with LED control is connected.
settings-leds-add = Add LED
settings-language = Language
settings-reopen-last = Reopen the last profile on startup
theme-light = Light
theme-dark = Dark
theme-system = System
theme-system-tip = Follow the system light or dark setting
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
virtual-banner = This profile uses virtual devices this system is missing or has set up smaller: { $devices }
virtual-setup = Set up
virtual-setup-tip = Create the missing virtual devices and add what the existing ones lack. Nothing is removed.
virtual-setup-admin = Create the missing virtual devices and add what the existing ones lack. Nothing is removed. Each device asks for administrator rights.
virtual-setup-active = Stop the profile to change virtual devices.
virtual-setup-done = Virtual devices are set up for the profile.
access-banner = Joystick Gobloon needs permission to read game controllers and create virtual devices.
access-grant = Grant access
access-granted = Device access granted.
access-error = Could not grant device access: { $error }
keyboard-access-banner = Keyboard bindings need permission to read the keyboards. Any program you run can then read what you type.
keyboard-access-allow = Allow keyboard input
keyboard-access-remove = Remove keyboard access
keyboard-access-allowed = Keyboard input allowed.
keyboard-access-removed = Keyboard access removed. It ends when the keyboard is reconnected or the computer restarts.
keyboard-access-error = Could not change keyboard access: { $error }
all-buttons-banner = { $device } has { $total } buttons, but Linux passes on only the first { $shown }. Programs that read the raw device, such as Steam, can then read this stick too, even while it is hidden.
all-buttons-allow = Read all buttons
all-buttons-allowed = All buttons of the stick are read now.
all-buttons-error = Could not allow reading all buttons: { $error }
subst-keep = Keep as is
subst-apply = Apply
status-substituted = Controls mapped. Save the profile to keep the change.

# vJoy
vjoy-missing = vJoy is not installed or could not be loaded.
vjoy-no-access = No permission to create vJoy devices.
vjoy-enabled = Driver enabled
vjoy-disabled = Driver disabled
vjoy-no-devices = No vJoy devices configured yet.
vjoy-axes = Axes
vjoy-buttons = Buttons
vjoy-hats = Hats
vjoy-test-hint = Test output: lock the device to use the controls below.
vjoy-test-pending = Apply the changes to test them.
vjoy-lock = Take exclusive control of this device to test its output
vjoy-unlock = Release the device
vjoy-free = Free
vjoy-own = Controlled by Joystick Gobloon
vjoy-busy = Used by another program or another copy of Joystick Gobloon
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
hidhide-confirm-on-title = Turn device hiding on?
hidhide-confirm-on-text = Hidden devices are seen only by the programs on the whitelist.
hidhide-confirm-off-title = Turn device hiding off?
hidhide-confirm-off-text = Every program sees all devices again, hidden ones too.
hidhide-confirm-hint = Double-click the eye to switch it without this question.
action-turn-on = Turn on
action-turn-off = Turn off
hidhide-hint = Click the eye to hide a device from other programs (closed eye). Reconnect it afterwards for the change to take effect.
hidhide-exposed = Still visible to programs that read the raw device (hidraw), e.g. Steam or Wine/Proton: closing it needs the administrator password, which was not given. Show and hide the device again to be asked again.
hidhide-no-devices = No devices to show.
action-refresh = Refresh
settings-see-hidden = See hidden devices
settings-see-hidden-tip = On startup the program adds itself to the programs that still see hidden devices, so profiles keep working.
settings-group-startup = Startup
settings-group-devices = Devices
settings-group-editor = Editor
settings-focus-input = Follow the input in use
settings-focus-input-tip = When an axis, button or hat is used, the Devices and bindings page selects its device tab and scrolls to it.
settings-focus-skip-modifiers = Stay in place on modifiers
settings-focus-skip-modifiers-tip = Pressing a modifier button or key does not scroll to it, so the input you are setting up stays in view.
settings-input-events = Read controllers on change
settings-input-events-tip = Lower latency and less CPU: controllers are read when they report a change, and short button presses are never missed. Off: every controller is read at the poll interval.
settings-poll-interval = Poll interval
settings-poll-interval-tip = How often a controller is polled: every controller when "Read controllers on change" is off, otherwise only the ones that cannot report changes. Shorter means lower latency and a little more CPU.
settings-poll-interval-unused = Not used now: every connected controller reports its changes, so none is polled. It applies when "Read controllers on change" is off or a controller cannot report changes.
settings-monotonic-curves = Curves only rise or fall
settings-monotonic-curves-tip = Response curve editor: a knot or handle stops where the curve would turn back, so it crosses zero only once. Curves that already turn back are edited freely.
settings-curve-right-click-invert = Right-click inverts curves
settings-curve-right-click-invert-tip = Response curve editor: a right click on empty space inverts the curve. The Invert button always does.
settings-warn-gremlin = Warn when saving changes the profile format
settings-warn-gremlin-tip = Profiles with modifiers or layers are saved in a format Joystick Gremlin cannot open.
gremlin-format-title = Joystick Gremlin will not open it
gremlin-format-text = { $file } uses features only Joystick Gobloon has (modifiers and layers). Once saved, Joystick Gremlin cannot open it any more.
gremlin-format-dont-ask = Don't ask again
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
edit-rename = Click to rename
edit-description-hint = Click to write a note
edit-click-to-edit = Click to edit

# Files and modes
action-new = New profile
wizard-open = New profile with the wizard: map each controller to a virtual joystick
wizard-title = New profile wizard
wizard-name = Profile name
wizard-name-hint = e.g. My flight setup
wizard-create = Create
wizard-devices = Pick the virtual joystick each controller maps to. Every axis, button and hat gets a Map to vJoy.
wizard-none = Not mapped
wizard-taken = Another controller maps to this virtual joystick
wizard-no-vjoy = No virtual joysticks yet: add them on the Virtual joysticks page first
wizard-fewer = { $vjoy } has fewer inputs than { $device }: { $missing } will not be mapped
wizard-axes = { $n ->
    [one] 1 axis
   *[other] { $n } axes
}
wizard-buttons = { $n ->
    [one] 1 button
   *[other] { $n } buttons
}
wizard-hats = { $n ->
    [one] 1 hat
   *[other] { $n } hats
}
status-wizard = Profile created: { $name }
action-save-as = Save as…
status-new = New profile
unsaved-title = Unsaved changes
unsaved-text = Save changes to { $file }?
unsaved-save = Save
unsaved-discard = Don't save
cancel = Cancel
modes-edit = Edit modes
modifiers-edit = Modifiers
modifiers-none = No modifiers yet. A modifier is a button that, while held, switches the other inputs to another layer of actions.
modifiers-add = Add a modifier
modifiers-press = Press a button on a controller…
modifiers-assign = Click, then press the button this modifier should use
modifiers-remove = Remove the modifier and the actions of its layers
modifier-input = Modifier { $name }: while held, the other inputs use its layer; it runs no actions itself
layer-base = Base
layer-set = This layer has actions
layer-empty = No actions on this layer
layer-additive = Add to the base
layer-additive-tip = On: these actions run together with the base layer. Off: they replace it while the modifiers are held.
layer-keep-value = Keep the last value
layer-keep-value-tip = On: when this layer stops, its axes keep their last value. Off: they go back to rest.
layers-tip = Layers: { $layers }
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
edit-curve-help = Drag a point or handle to move it · Double-click to add a point · Right-click a point to remove it
edit-curve-help-invert = Right-click empty space to invert

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
vjoy-quickmap = Pick an output
output-profile = Profile: { $name }
output-free = { $output }: free
output-used = { $output }: { $n ->
    [one] used by 1 input
   *[other] used by { $n } inputs
}
output-conflict = Inputs that run at the same time write it
output-more = …and { $n } more
output-legend-used = Used
output-legend-conflict = Conflict
output-legend-tip = Show these controls (the profile does not change)
output-same-input = Another action of this input already writes it
output-same-input-clash = Another action of this input writes the same output: pick a different one
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
status-latency = Latency from reading a controller to the virtual output, last second: average { $average } ms, max { $max } ms. The controller's own delay is not included.

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
edit-merge-full = It already merges two other axes; change one of them in its editor.
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
edit-tts-front = Queue next (the system's speech: same as after everything else)
edit-tts-interrupt = Interrupt what is being said
edit-preview = Preview

kind-light-led = Light LED
edit-led-device = Joystick with LEDs
edit-led-no-device = This joystick is not connected or its LEDs cannot be controlled
edit-led-id = LED
led-vkb-grip = RGB on top of the grip
led-vkb-hat = Small red at the hat
edit-led-color = Color

edit-led-color2 = Second color
edit-led-blend = Colors
led-blend-color1 = One color
led-blend-color2 = Second color only
led-blend-alternate = Alternate colors
led-blend-alternate-reverse = Alternate, second color first
led-blend-mix = Mix colors
led-vkb-base-blue = Base, blue (right)
led-vkb-base-red = Base, red (left)
edit-led-mode = How the LED shines
led-mode-on = On
led-mode-slow-blink = Slow blink
led-mode-fast-blink = Fast blink
led-mode-fastest-blink = Very fast blink
led-mode-off = Off
edit-led-test = Light it now
edit-led-on-activate = When the profile or mode is activated
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
settings-tray-double-click = Double click activates the profile
settings-tray-title = Tray icon
settings-autostart = Start at logon (minimized)
settings-activate-on-start = Activate the profile on startup
tray-show = Show Joystick Gobloon
tray-hiding = Device hiding
tray-activate = Activate “{ $name }”
tray-activate-none = Activate the profile
tray-quit = Quit
autostart-error = Could not change starting at logon: { $error }

# About
about-alpha = alpha build
about-created = Created by { $name }
about-support = Support development
about-support-tip = Contribute via PayPal
about-source = Source code
about-issues = Report a problem
about-releases = Releases
about-hiding = Device hiding: { $backend }
about-virtual = Virtual joysticks: { $backend }
about-not-installed = not installed
about-gremlin = Profiles are compatible with Joystick Gremlin.
about-license = Free and open source under the MIT license.

# Updates
settings-check-updates = Check for a new version on startup and install it
update-check = Check for updates
update-checking = Looking for a new version…
update-latest = You have the latest version
update-available = Version { $version } is available
update-install = Update now
update-downloading = Downloading version { $version }…
update-error = Could not check for updates
update-debug = Debug builds do not update themselves
update-install-error = Could not install the update: { $error }

# Input viewer
viewer-history = Axes over time
viewer-current = Axes now
viewer-buttons = Buttons and hats
viewer-axis = Axis { $n }

# vJoy configuration
vjoy-add = Add a vJoy device
vjoy-add-max = vJoy supports at most 16 devices.
vjoy-new-device = New vJoy #{ $n }: not created yet
vjoy-enable = Enable the vJoy driver
vjoy-disable = Disable the vJoy driver
vjoy-hat-continuous = Continuous
vjoy-hat-discrete = 4 directions
vjoy-ffb = Force feedback
vjoy-ffb-enable = Enabled
vjoy-name = Name
vjoy-name-suggest = Suggest a name: the controller the profile maps onto this device, or its layout
vjoy-name-invalid = Up to 79 characters, no spaces at the ends, different from the other devices' names.
vjoy-name-renamed = Games see a renamed device as a new one: their bindings to it may need setting again.
vjoy-product = Product ID
vjoy-product-tip = Games tell devices apart by it. Empty = picked by the device number; enter the ID of an old setup to keep a game's bindings.
vjoy-product-invalid = A hex number from BE01 to BEFF (except BEAD), not used by another device.
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
curve-monotone = Smooth, no overshoot
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
calibration-jitter = Jitter filter
calibration-jitter-tip = Ignores changes smaller than this share of the travel, for worn axes that jitter. No delay; the ends and the center are always reached. The line on the bar shows what is sent. Save to apply
calibration-debounce = Debounce
calibration-debounce-tip = Buttons and hats of this device: after a press or release, further changes within this time are ignored, for switches that bounce. The first change passes at once
calibration-in-window = The calibration is open in its own window.

# Device info
device-info = Device information

device-leds = LEDs can be controlled (Light LED action)
device-info-name = Device name
device-info-axes = Axes
device-info-buttons = Buttons
device-info-hats = Hats
device-info-vid = VID
device-info-pid = PID
device-info-joystick-id = Joystick ID
device-info-guid = Device GUID
device-info-copy = Copy to clipboard

# Virtual gamepads
kind-map-to-gamepad = Map to gamepad
gamepad-n = Gamepad { $n }
gamepad-tip = Xbox-style virtual gamepad: games, Steam and Proton see an Xbox 360 controller.
gamepad-add = Add a virtual gamepad
gamepad-add-max = At most { $n } gamepads.
gamepad-add-n = Add gamepad { $n }
gamepad-delete-n = Delete gamepad { $n }
gamepad-missing = Gamepad { $n } does not exist yet: add it on the Virtual joysticks page.
pad-a = A
pad-b = B
pad-x = X
pad-y = Y
pad-lb = LB
pad-rb = RB
pad-back = Back
pad-start = Start
pad-guide = Guide
pad-ls = Left stick press
pad-rs = Right stick press
pad-dpad-up = D-pad up
pad-dpad-down = D-pad down
pad-dpad-left = D-pad left
pad-dpad-right = D-pad right
pad-left-x = Left stick X
pad-left-y = Left stick Y
pad-right-x = Right stick X
pad-right-y = Right stick Y
pad-lt = LT (left trigger)
pad-rt = RT (right trigger)
pad-ls-short = LS
pad-rs-short = RS
pad-dpad = D-pad

system-banner-install = Install System Gobloon: hidden sticks are hidden from every program, and virtual devices stay while games run, as with HidHide and vJoy on Windows.
system-banner-stopped = System Gobloon is installed but not running.
system-banner-update = System Gobloon { $version } is installed; this program brings another version.
system-install = Install
system-update = Update
system-start = Start
system-remove = Remove
system-not-now = Not now
system-admin = Asks for the administrator password
system-about = Hides sticks from every program and keeps the virtual joysticks, gamepads, keyboard and mouse while games run. No password is needed after installing.
system-status-missing = Not installed.
system-status-stopped = Installed, not running.
system-status-outdated = Version { $version } runs: update it to this program's version.
system-status-current = Version { $version } runs.
system-remove-title = Remove System Gobloon?
system-remove-text = Hidden sticks are then hidden only while Joystick Gobloon runs, and virtual devices exist only while a profile is active. The hidden devices and the virtual device layouts are kept.
system-installed = System Gobloon is installed.
system-removed = System Gobloon is removed.
system-error = System Gobloon: { $error }
vjoy-not-persistent = These devices exist only while a profile is active or a test holds them. System Gobloon (General settings) keeps them all the time.
hidhide-while-running = Devices are hidden only while Joystick Gobloon runs. System Gobloon (General settings) hides them from every program, all the time.
