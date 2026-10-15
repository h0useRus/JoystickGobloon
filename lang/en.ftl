language-name = English

# Navigation and toolbar tooltips
nav-devices = Devices and bindings
nav-hidhide = Device hiding
nav-vjoy = Virtual joysticks
nav-settings = Settings
action-open = Import Joystick Gremlin profile
action-save = Save
action-report = Import report
label-mode = Mode

# Settings
settings-appearance = Appearance
settings-general = General
settings-about = About
settings-language = Language
settings-reopen-last = Reopen the last profile on startup
theme-light = Light
theme-dark = Dark
about-text = Device hiding, virtual joysticks and input mapping in one app.

# Empty states
no-profile = No profile loaded
device-linked = Bindings of the profile device with the same name. Use “Replace devices” to save the link.
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
import-title = Import report
import-summary = Format version { $version }: { $inputs } inputs, { $actions } actions, { $devices } devices, { $modes } modes.
import-no-warnings = No problems found.
warn-dangling = Missing action { $to } (referenced from { $from })
warn-unknown-mode = Inputs use the undeclared mode “{ $mode }”
warn-not-executable = “{ $kind }” × { $count }: kept in the profile, but not executed yet
warn-scripts = The profile contains Python plugins: they are kept, but not run

# Device substitution
subst-title = Replace devices
subst-banner = Device IDs in the profile do not match this PC: { $count }
subst-keep = Keep as is
subst-apply = Apply
status-substituted = Devices replaced. Save the profile to keep the change.
profile-modified = Unsaved changes

# vJoy
vjoy-missing = vJoy is not installed or could not be loaded.
vjoy-enabled = Driver enabled
vjoy-disabled = Driver disabled
vjoy-no-devices = No vJoy devices configured. Add one in “Configure vJoy”.
vjoy-axes = Axes
vjoy-buttons = Buttons
vjoy-hats = Hats
vjoy-test = Test output: take control of this device
vjoy-test-hint = Press the flask to take control of this device and test its output.
vjoy-free = Free
vjoy-own = Controlled by JoyGob
vjoy-busy = Used by another program (e.g. Joystick Gremlin)
vjoy-unknown = Unknown state
vjoy-error = Could not take the device: { $error }

# Device hiding
hidhide-missing = HidHide is not installed.
hidhide-toggle = Turn device hiding on or off
hidhide-on = Hiding is on
hidhide-off = Hiding is off
hidhide-devices = Devices
hidhide-apps = Applications
hidhide-hidden = Hidden: only whitelisted applications see it
hidhide-visible = Visible to every application
hidhide-add-app = Add application
hidhide-remove-app = Remove from the whitelist
hidhide-app-missing = File not found
action-refresh = Refresh
settings-hidhide-self = Add JoyGob to the HidHide whitelist on startup
status-whitelisted = JoyGob was added to the HidHide whitelist.

# Engine
action-activate = Activate the profile
action-deactivate = Stop the profile
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
modes-title = Modes
modes-name = Name
modes-parent = Inherits from
modes-startup = Mode on startup
modes-remove = Remove mode and its bindings
modes-new = New mode
modes-add = Add mode

# Undo
action-undo = Undo (Ctrl+Z)
action-redo = Redo (Ctrl+Y)
status-undone = Undone
status-redone = Redone
edit-curve-help = Drag a point to move it · Click empty space to invert · Double-click to add a point · Right-click a point to remove it
edit-curve-select = Click a point to edit its position.

# vJoy mapping
vjoy-device-n = vJoy Device { $n }
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
