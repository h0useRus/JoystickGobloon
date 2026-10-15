language-name = English

# Navigation and toolbar tooltips
nav-devices = Devices and bindings
nav-hidhide = Device hiding
nav-vjoy = Virtual joysticks
nav-settings = Settings
action-open = Import Joystick Gremlin profile
action-save = Save as Joystick Gremlin profile
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
placeholder-hidhide = Hide physical devices from games (HidHide driver).

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
