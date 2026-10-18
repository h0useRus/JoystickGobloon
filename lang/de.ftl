language-name = Deutsch

# Navigation and toolbar tooltips
nav-devices = Geräte und Belegungen
nav-hidhide = Geräte verbergen
nav-apps = Anwendungen
nav-vjoy = Virtuelle Joysticks
nav-viewer = Eingabeanzeige
nav-calibration = Kalibrierung
nav-settings = Einstellungen
action-open = Joystick-Gremlin-Profil importieren
action-save = Speichern
label-mode = Modus

# Settings
settings-appearance = Darstellung
settings-general = Allgemein
settings-about = Info
settings-language = Sprache
settings-reopen-last = Beim Start das letzte Profil öffnen
theme-light = Hell
theme-dark = Dunkel
theme-system = System
theme-system-tip = Der hellen oder dunklen Windows-Einstellung folgen
about-text = Geräte verbergen, virtuelle Joysticks und Eingabebelegung in einer App.

# Empty states
no-profile = Kein Profil geladen
device-linked = Belegungen des Profilgeräts mit demselben Namen. Mit „Steuerungen zuordnen“ die Verknüpfung speichern.
device-disconnected = Nicht verbunden
no-devices-attached = Keine Gamecontroller gefunden

# Status bar
status-ready = Bereit
status-loaded = { $file } geladen
status-saved = { $file } gespeichert
import-error = Das Profil konnte nicht geöffnet werden: { $error }
save-error = Das Profil konnte nicht gespeichert werden: { $error }
file-filter-gremlin = Joystick-Gremlin-Profil

# Import report
alert-error = Etwas ist schiefgelaufen
alert-import = Einige Aktionen in { $file } funktionieren nicht
alert-open-log = Protokoll öffnen
ok = OK
hidhide-whitelist-error = Joystick Gobloon konnte nicht zur HidHide-Positivliste hinzugefügt werden: { $error }
warn-not-executable = „{ $kind }“ × { $count }: im Profil behalten, aber noch nicht ausgeführt
warn-scripts = Das Profil enthält Python-Plugins: Sie bleiben erhalten, werden aber nicht ausgeführt

# Map controls (device substitution)
subst-title = Steuerungen zuordnen
subst-auto = Automatisch
subst-auto-tip = Die übrigen Geräte angeschlossenen Geräten mit ähnlichen Namen zuordnen; unklare bleiben zur Auswahl
subst-banner = Geräte-IDs im Profil passen nicht zu diesem PC: { $count }
subst-keep = Unverändert lassen
subst-apply = Übernehmen
status-substituted = Steuerungen zugeordnet. Speichern Sie das Profil, um die Änderung zu behalten.

# vJoy
vjoy-missing = vJoy ist nicht installiert oder konnte nicht geladen werden.
vjoy-enabled = Treiber aktiviert
vjoy-disabled = Treiber deaktiviert
vjoy-no-devices = Noch keine vJoy-Geräte eingerichtet.
vjoy-axes = Achsen
vjoy-buttons = Tasten
vjoy-hats = Coolie-Hats
vjoy-test-hint = Ausgabe testen: Gerät sperren, um die Steuerelemente unten zu benutzen.
vjoy-test-pending = Übernehmen Sie die Änderungen, um sie zu testen.
vjoy-lock = Dieses Gerät exklusiv übernehmen, um seine Ausgabe zu testen
vjoy-unlock = Gerät freigeben
vjoy-free = Frei
vjoy-own = Von Joystick Gobloon gesteuert
vjoy-busy = Von einem anderen Programm verwendet (z. B. Joystick Gremlin)
vjoy-unknown = Unbekannter Zustand
vjoy-error = Das Gerät konnte nicht übernommen werden: { $error }

# Device hiding
hidhide-missing = HidHide ist nicht installiert.
hidhide-on = Geräte verbergen ist an
hidhide-off = Geräte verbergen ist aus
hidhide-hidden = Verborgen: nur Anwendungen auf der Positivliste sehen es
hidhide-visible = Für alle Anwendungen sichtbar
hidhide-filter-disconnected = Getrennte Geräte ausblenden
hidhide-gaming-only = Nur Gamecontroller
hidhide-enable = Geräte verbergen aktivieren
hidhide-hint = Klicken Sie auf das Auge, um ein Gerät vor anderen Programmen zu verbergen (geschlossenes Auge). Verbinden Sie es danach neu, damit die Änderung wirkt.
hidhide-no-devices = Keine Geräte anzuzeigen.
action-refresh = Aktualisieren
settings-hidhide-self = Joystick Gobloon beim Start zur HidHide-Positivliste hinzufügen
status-whitelisted = Joystick Gobloon wurde zur HidHide-Positivliste hinzugefügt.

# Applications
apps-add = Anwendung hinzufügen
apps-empty = Noch keine Anwendungen. Fügen Sie ein Spiel oder Programm hinzu, um es einzurichten.
apps-missing = Datei nicht gefunden
apps-missing-text = Das Programm ist nicht mehr vorhanden. Aus der Liste entfernen?
apps-sees = Sieht verborgene Geräte
apps-blind = Sieht verborgene Geräte nicht
apps-inverse = Anwendungsliste umkehren
apps-inverse-tip = Aufgeführte Programme sehen verborgene Geräte nicht; alle anderen schon
apps-remove = Aus der Liste entfernen

# Engine
action-activate = Profil aktivieren
action-deactivate = Profil stoppen
title-active = Aktiv
title-inactive = Inaktiv
status-active = Profil aktiv
status-stopped = Profil gestoppt
activate-error = Das Profil konnte nicht aktiviert werden: { $error }

# Editing
edit-add = Aktion hinzufügen
edit-remove = Entfernen
edit-up = Nach oben
edit-down = Nach unten
edit-not-editable = Diese Aktion bleibt im Profil erhalten, kann aber noch nicht bearbeitet werden.
edit-on-press = Beim Drücken
edit-on-release = Beim Loslassen
edit-on-both = Beim Drücken und Loslassen
edit-inverted = Invertiert
edit-threshold = Schwelle für langes Drücken
edit-short = Kurz drücken
edit-long = Lang drücken
edit-deadzone = Totzone
edit-invert = Invertieren
edit-inherited = Geerbt vom Modus „{ $mode }“
edit-unbind = Alle Aktionen dieser Eingabe entfernen
edit-override = In diesem Modus überschreiben
input-axis-kind = Achse
input-button-kind = Taste
input-hat-kind = Coolie-Hat
kind-map-to-vjoy = Auf vJoy abbilden
kind-response-curve = Kennlinie
kind-change-mode = Modus wechseln
kind-tempo = Kurz / lang drücken
kind-description = Beschreibung

# Files and modes
action-new = Neues Profil
action-save-as = Speichern unter…
status-new = Neues Profil
unsaved-title = Ungespeicherte Änderungen
unsaved-text = Änderungen an { $file } speichern?
unsaved-save = Speichern
unsaved-discard = Nicht speichern
cancel = Abbrechen
modes-edit = Modi bearbeiten
modes-title = Modi
modes-name = Name
modes-parent = Erbt von
modes-remove = Modus und seine Belegungen entfernen
modes-new = Neuer Modus
modes-add = Modus hinzufügen

# Undo
action-undo = Rückgängig (Strg+Z)
action-redo = Wiederholen (Strg+Y)
status-undone = Rückgängig gemacht
status-redone = Wiederholt
edit-curve-help = Punkt oder Griff ziehen, um ihn zu verschieben · Klick auf freie Fläche invertiert · Doppelklick fügt einen Punkt hinzu · Rechtsklick auf einen Punkt entfernt ihn

# vJoy mapping
vjoy-device-n = vJoy #{ $n }
vjoy-axis-1 = X-Achse
vjoy-axis-2 = Y-Achse
vjoy-axis-3 = Z-Achse
vjoy-axis-4 = X-Rotation
vjoy-axis-5 = Y-Rotation
vjoy-axis-6 = Z-Rotation
vjoy-axis-7 = Schieberegler
vjoy-axis-8 = Drehregler
vjoy-button-n = Taste { $n }
vjoy-hat-n = Coolie-Hat { $n }
vjoy-absolute = Absolut
vjoy-relative = Relativ
vjoy-speed = Geschwindigkeit

# Mouse
kind-map-to-mouse = Auf Maus abbilden
mouse-mode-button = Maustaste
mouse-mode-motion = Zeigerbewegung
mouse-left = Linke Taste
mouse-right = Rechte Taste
mouse-middle = Mittlere Taste
mouse-forward = Vorwärts-Taste
mouse-back = Zurück-Taste
mouse-wheel-up = Rad nach oben
mouse-wheel-down = Rad nach unten
mouse-direction = Richtung
mouse-min-speed = Mindestgeschwindigkeit
mouse-max-speed = Höchstgeschwindigkeit
mouse-time-to-max = Zeit bis zur Höchstgeschwindigkeit

# Toggle, split, pause, run
kind-smart-toggle = Intelligenter Umschalter
kind-split-axis = Achse teilen
kind-pause-resume = Pausieren / fortsetzen
kind-run-command = Befehl ausführen
edit-delay = Haltezeit für Tastbetrieb
edit-split = Teilen bei
edit-lower = Unterer Teil
edit-upper = Oberer Teil
pause-op-pause = Pausieren
pause-op-resume = Fortsetzen
pause-op-toggle = Umschalten
run-executable = Programm
run-arguments = Argumente
run-browse = Programm auswählen
status-paused = Profil pausiert

# Double tap, chain
kind-double-tap = Doppeltippen
kind-chain = Kette
edit-tap-threshold = Zeit bis zum zweiten Tippen
edit-tap-exclusive = Exklusiv: einfaches Tippen wartet, um Doppeltippen auszuschließen
edit-tap-combined = Kombiniert: Doppeltippen führt auch die Aktionen für einfaches Tippen aus
edit-single = Einfaches Tippen
edit-double = Doppeltippen
edit-chain-timeout = Nach dieser Ruhezeit wieder mit dem ersten Schritt beginnen (0 = nie)
edit-chain-step = Schritt { $n }

# Hat as buttons
kind-hat-buttons = Coolie-Hat als Tasten
edit-hat-four = 4 Richtungen
edit-hat-eight = 8 Richtungen (mit Diagonalen)
hat-north = Norden
hat-north-east = Nordosten
hat-east = Osten
hat-south-east = Südosten
hat-south = Süden
hat-south-west = Südwesten
hat-west = Westen
hat-north-west = Nordwesten
hat-center = Mitte

# Merge axis
kind-merge-axis = Achsen zusammenführen
edit-reuse = Diese vorhandene Aktion auch hier hinzufügen (geteilt, wie Gremlin sie an beide Achsen bindet)
edit-merge-name = Name
edit-merge-no-device = (kein Gerät)
edit-merged = Zusammengeführte Achse
merge-average = Durchschnitt
merge-minimum = Minimum
merge-maximum = Maximum
merge-sum = Summe
merge-bidirectional = Bidirektional (Achse 2 minus Achse 1)
merge-maximum-deflection = Größter Ausschlag
merge-closest-to-center = Am nächsten zur Mitte

# Axis delta
kind-axis-delta = Achsenänderung
edit-delta-threshold = Achsenänderung, die einen Impuls auslöst
edit-delta-positive = Achse gestiegen
edit-delta-negative = Achse gesunken

# Dual axis deadzone
kind-dual-axis-deadzone = Totzone für zwei Achsen
edit-dual-inner = Innere Totzone (Kreis um die Mitte)
edit-dual-outer = Äußere Grenze (ab hier voller Ausschlag)
edit-dual-close = Innere und äußere Totzone liegen zu dicht beieinander.
edit-dual-first = Ausgabe der ersten Achse
edit-dual-second = Ausgabe der zweiten Achse

# Existing actions (Gremlin's reference action)
edit-link-existing = Vorhandene Aktion verknüpfen
edit-copy-existing = Vorhandene Aktion kopieren

# Condition
kind-condition = Bedingung
edit-cond-all = Alle Bedingungen müssen erfüllt sein
edit-cond-any = Eine Bedingung genügt
edit-cond-add = Bedingung
edit-cond-true = Wenn wahr
edit-cond-false = Sonst
cond-current = Diese Eingabe
cond-joystick = Joystick-Eingabe
cond-vjoy = vJoy-Ausgabe
cond-keyboard = Gehaltene Tasten
cond-range = Achse zwischen
cond-pressed = Gedrückt
cond-released = Losgelassen

# Sound, speech, load profile
kind-play-sound = Ton abspielen
kind-text-to-speech = Sprachausgabe
kind-load-profile = Profil laden
file-filter-sound = Tondatei
edit-volume = Lautstärke
edit-file-missing = Datei nicht gefunden
edit-tts-hint = Zu sprechender Text; $current_mode wird durch den aktiven Modus ersetzt
edit-tts-rate = Geschwindigkeit
edit-tts-pitch = Tonhöhe
edit-tts-back = Hinter allem anderen einreihen
edit-tts-front = Als Nächstes einreihen (Windows-Sprachausgabe: wie hinter allem anderen)
edit-tts-interrupt = Laufende Ansage unterbrechen
edit-preview = Vorschau
edit-tts-voice = Stimme
edit-tts-default-voice = Standardstimme
edit-tts-voice-missing = nicht installiert
kind-map-to-keyboard = Auf Tastatur abbilden

# Key recording
keys-record = Tasten aufnehmen: gemeinsam drücken, die Aufnahme endet, wenn alle losgelassen sind
keys-press = Tasten drücken…
keys-none = Keine Tasten
keys-cancel = Aufnahme abbrechen
edit-chain-add = Schritt hinzufügen
edit-chain-remove = Diesen Schritt und seine Aktionen entfernen
kind-macro = Makro

# Macro editor
macro-repeat = Wiederholen
macro-repeat-single = Einmal
macro-repeat-count = Anzahl
macro-repeat-hold = Solange gehalten
macro-repeat-toggle = Bis zum nächsten Drücken
macro-repeat-count-tip = Wie oft
macro-repeat-delay = Pause zwischen Wiederholungen
macro-add-step = Schritt
macro-step-key = Taste
macro-step-pause = Pause
macro-step-mouse-button = Maustaste
macro-step-mouse-motion = Mausbewegung
macro-step-vjoy-button = vJoy-Taste
macro-step-vjoy-axis = vJoy-Achse
macro-step-vjoy-hat = vJoy-Coolie-Hat
macro-press = Drücken
macro-release = Loslassen

# Logical device
logical-device = Logisches Gerät
logical-device-tip = Virtuelle Eingaben dieses Profils: Eingaben auf sie abbilden und sie dann wie einen Stick belegen
logical-add-axis = Achse hinzufügen
logical-add-button = Taste hinzufügen
logical-add-hat = Coolie-Hat hinzufügen
logical-remove = Diese Eingabe und ihre Belegungen entfernen
logical-empty = Noch keine logischen Eingaben: fügen Sie mit den Schaltflächen oben eine hinzu.
kind-map-to-logical-device = Auf logisches Gerät abbilden
cond-logical = Eingabe des logischen Geräts

# Tray and startup
settings-tray = Symbol im Infobereich anzeigen
settings-close-to-tray = Beim Schließen in den Infobereich
settings-minimize-to-tray = Beim Minimieren in den Infobereich
settings-tray-title = Symbol im Infobereich
settings-autostart = Mit Windows starten (minimiert)
settings-activate-on-start = Profil beim Start aktivieren
tray-show = Joystick Gobloon anzeigen
tray-hiding = Geräte verbergen
tray-activate = „{ $name }“ aktivieren
tray-activate-none = Profil aktivieren
tray-quit = Beenden
autostart-error = Der Start mit Windows konnte nicht geändert werden: { $error }

# About
about-alpha = Alpha-Version
about-created = Erstellt von { $name }
about-support = Entwicklung unterstützen
about-support-tip = Über PayPal beitragen
about-source = Quellcode
about-issues = Problem melden
about-releases = Versionen
about-credits = Verwendet die Treiber HidHide und vJoy; Profile sind mit Joystick Gremlin kompatibel.
about-license = Freie Open-Source-Software unter der MIT-Lizenz.

# Updates
settings-check-updates = Beim Start nach einer neuen Version suchen und sie installieren
update-check = Nach Updates suchen
update-checking = Suche nach einer neuen Version…
update-latest = Sie haben die neueste Version
update-available = Version { $version } ist verfügbar
update-install = Jetzt aktualisieren
update-downloading = Version { $version } wird heruntergeladen…
update-error = Die Suche nach Updates ist fehlgeschlagen
update-debug = Debug-Versionen aktualisieren sich nicht
update-install-error = Das Update konnte nicht installiert werden: { $error }

# Input viewer
viewer-history = Achsen im Zeitverlauf
viewer-current = Achsen jetzt
viewer-buttons = Tasten und Coolie-Hats
viewer-axis = Achse { $n }

# vJoy configuration
vjoy-add = vJoy-Gerät hinzufügen
vjoy-add-max = vJoy unterstützt höchstens 16 Geräte.
vjoy-new-device = Neues vJoy #{ $n }: noch nicht erstellt
vjoy-enable = vJoy-Treiber aktivieren
vjoy-disable = vJoy-Treiber deaktivieren
vjoy-hat-continuous = Stufenlos
vjoy-hat-discrete = 4 Richtungen
vjoy-ffb = Force Feedback
vjoy-ffb-enable = Aktiviert
vjoy-ffb-const = Konstant
vjoy-ffb-ramp = Rampe
vjoy-ffb-sq = Rechteck
vjoy-ffb-sine = Sinus
vjoy-ffb-tr = Dreieck
vjoy-ffb-stup = Sägezahn aufwärts
vjoy-ffb-stdn = Sägezahn abwärts
vjoy-ffb-spr = Feder
vjoy-ffb-dm = Dämpfer
vjoy-ffb-inr = Trägheit
vjoy-ffb-fric = Reibung
vjoy-config-create = Erstellen
vjoy-config-apply = Übernehmen
vjoy-config-revert = Zurücksetzen
vjoy-config-remove = Gerät entfernen
vjoy-config-admin = Windows fragt nach Administratorrechten, um vJoy zu ändern
vjoy-config-locked = Stoppen Sie das Profil, um vJoy-Geräte zu ändern.
vjoy-config-reading = vJoy-Konfiguration wird gelesen...
vjoy-config-label = vJoy #{ $n } einrichten
vjoy-config-done = vJoy-Konfiguration geändert.
vjoy-config-error = Die vJoy-Konfiguration konnte nicht geändert werden: { $error }
vjoy-config-reboot = vJoy-Konfiguration geändert. Starten Sie Windows neu, damit sie wirksam wird.
vjoy-remove-title = vJoy #{ $n } entfernen?
vjoy-remove-text = Spiele und Profile, die dieses Gerät verwenden, verlieren es, bis es neu erstellt wird.

# Response curve
curve-type = Kurventyp
curve-linear = Stückweise linear
curve-cubic = Kubischer Spline
curve-bezier = Kubischer Bézier-Spline
curve-symmetric = Symmetrisch
curve-symmetric-tip = Kurve beim Bearbeiten am Mittelpunkt spiegeln
curve-point = Punkt
curve-handle = Griff

# Keyboard device
keyboard-device = Tastatur
keyboard-tip = Tastaturtasten als Eingaben: Aktionen an Tasten binden wie an Joystick-Tasten
keyboard-add = Taste belegen
keyboard-empty = Noch keine Tasten belegt: nehmen Sie mit der Schaltfläche oben eine auf.
edit-binding-description = Beschreibung dieser Eingabe
edit-unbound = Noch keine Aktionen: fügen Sie eine hinzu.
viewer-detach = In eigenem Fenster öffnen
viewer-attach = Zurück ins Hauptfenster
viewer-in-window = Die Eingabeanzeige ist in einem eigenen Fenster geöffnet.

# Calibration
calibration-device = Zu kalibrierendes Gerät
calibration-raw = Rohwert
calibration-with-center = Mit Mitte
calibration-reset = Zurücksetzen
calibration-save = Diese Kalibrierung speichern
calibration-center = Mitte kalibrieren
calibration-center-tip = Stick ruhen lassen, leicht um die Mitte bewegen, dann erneut klicken
calibration-extrema = Endpunkte kalibrieren
calibration-extrema-tip = Achse an beide Enden bewegen, dann erneut klicken
calibration-in-window = Die Kalibrierung ist in einem eigenen Fenster geöffnet.

# Device info
device-info = Geräteinformationen
device-info-name = Gerätename
device-info-axes = Achsen
device-info-buttons = Tasten
device-info-hats = Coolie-Hats
device-info-vid = VID
device-info-pid = PID
device-info-joystick-id = Joystick-ID
device-info-guid = Geräte-GUID
device-info-copy = In die Zwischenablage kopieren
