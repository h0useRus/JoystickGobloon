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

settings-lighting = Beleuchtung
settings-leds-hint = Wird ein Profil deaktiviert, das die LEDs eines Joysticks geändert hat, werden diese Farben gesetzt. Der Joystick behält geänderte Farben bis zum erneuten Anschließen, seine eigenen LED-Einstellungen lassen sich daher nicht zurückholen.
settings-leds-none = Kein Joystick mit steuerbaren LEDs angeschlossen.
settings-leds-add = LED hinzufügen
settings-language = Sprache
settings-reopen-last = Beim Start das letzte Profil öffnen
theme-light = Hell
theme-dark = Dunkel
theme-system = System
theme-system-tip = Der hellen oder dunklen Systemeinstellung folgen
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
virtual-banner = Dieses Profil nutzt virtuelle Geräte, die hier fehlen oder kleiner eingerichtet sind: { $devices }
virtual-setup = Einrichten
virtual-setup-tip = Fehlende virtuelle Geräte anlegen und vorhandenen hinzufügen, was ihnen fehlt. Nichts wird entfernt.
virtual-setup-admin = Fehlende virtuelle Geräte anlegen und vorhandenen hinzufügen, was ihnen fehlt. Nichts wird entfernt. Jedes Gerät fragt nach Administratorrechten.
virtual-setup-active = Beende das Profil, um virtuelle Geräte zu ändern.
virtual-setup-done = Die virtuellen Geräte sind für das Profil eingerichtet.
access-banner = Joystick Gobloon braucht die Berechtigung, Gamecontroller zu lesen und virtuelle Geräte zu erstellen.
access-grant = Zugriff erteilen
access-granted = Gerätezugriff erteilt.
access-error = Gerätezugriff konnte nicht erteilt werden: { $error }
keyboard-access-banner = Tastaturbelegungen brauchen die Berechtigung, die Tastaturen zu lesen. Dann kann jedes Programm, das Sie starten, mitlesen, was Sie tippen.
keyboard-access-allow = Tastatureingabe erlauben
keyboard-access-remove = Tastaturzugriff entfernen
keyboard-access-allowed = Tastatureingabe erlaubt.
keyboard-access-removed = Tastaturzugriff entfernt. Er endet, sobald die Tastatur neu verbunden oder der Computer neu gestartet wird.
keyboard-access-error = Tastaturzugriff konnte nicht geändert werden: { $error }
all-buttons-banner = { $device } hat { $total } Tasten, aber Linux gibt nur die ersten { $shown } weiter. Programme, die das Gerät direkt lesen, etwa Steam, können diesen Stick dann ebenfalls lesen, auch wenn er versteckt ist.
all-buttons-allow = Alle Tasten lesen
all-buttons-allowed = Alle Tasten des Sticks werden jetzt gelesen.
all-buttons-error = Lesen aller Tasten konnte nicht erlaubt werden: { $error }
subst-keep = Unverändert lassen
subst-apply = Übernehmen
status-substituted = Steuerungen zugeordnet. Speichern Sie das Profil, um die Änderung zu behalten.

# vJoy
vjoy-missing = vJoy ist nicht installiert oder konnte nicht geladen werden.
vjoy-no-access = Keine Berechtigung, vJoy-Geräte zu erstellen.
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
vjoy-busy = Von einem anderen Programm oder einer anderen Joystick-Gobloon-Instanz verwendet
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
hidhide-confirm-on-title = Geräte verbergen einschalten?
hidhide-confirm-on-text = Verborgene Geräte sehen nur die Programme auf der Whitelist.
hidhide-confirm-off-title = Geräte verbergen ausschalten?
hidhide-confirm-off-text = Alle Programme sehen wieder alle Geräte, auch die verborgenen.
hidhide-confirm-hint = Doppelklick auf das Auge schaltet ohne diese Frage um.
action-turn-on = Einschalten
action-turn-off = Ausschalten
hidhide-hint = Klicken Sie auf das Auge, um ein Gerät vor anderen Programmen zu verbergen (geschlossenes Auge). Verbinden Sie es danach neu, damit die Änderung wirkt.
hidhide-exposed = Für Programme, die das Gerät direkt lesen (hidraw), z. B. Steam oder Wine/Proton, weiterhin sichtbar: Zum Sperren ist das Administratorkennwort nötig, es wurde nicht eingegeben. Blende das Gerät ein und wieder aus, um erneut gefragt zu werden.
hidhide-no-devices = Keine Geräte anzuzeigen.
action-refresh = Aktualisieren
settings-see-hidden = Verborgene Geräte sehen
settings-see-hidden-tip = Beim Start fügt sich das Programm zu den Programmen hinzu, die verborgene Geräte weiterhin sehen, damit Profile funktionieren.
settings-group-startup = Start
settings-group-devices = Geräte
settings-group-editor = Editor
settings-focus-input = Der benutzten Eingabe folgen
settings-focus-input-tip = Wird eine Achse, Taste oder ein Hat benutzt, wählt die Seite „Geräte und Belegungen“ den Tab des Geräts und scrollt dorthin.
settings-focus-skip-modifiers = Bei Modifikatoren nicht springen
settings-focus-skip-modifiers-tip = Das Drücken einer Modifikatortaste scrollt nicht zu ihr, die Eingabe, die du einrichtest, bleibt sichtbar.
settings-input-events = Controller bei Änderung lesen
settings-input-events-tip = Weniger Latenz und CPU-Last: Controller werden gelesen, wenn sie eine Änderung melden, und kurze Tastendrücke gehen nie verloren. Aus: jeder Controller wird im Abfrageintervall abgefragt.
settings-poll-interval = Abfrageintervall
settings-poll-interval-tip = Wie oft ein Controller abgefragt wird: alle Controller, wenn „Controller bei Änderung lesen“ aus ist, sonst nur die, die keine Änderungen melden können. Kürzer heißt weniger Latenz und etwas mehr CPU-Last.
settings-poll-interval-unused = Derzeit nicht verwendet: Jeder angeschlossene Controller meldet seine Änderungen, daher wird keiner abgefragt. Gilt, wenn „Controller bei Änderung lesen“ aus ist oder ein Controller keine Änderungen melden kann.
settings-monotonic-curves = Kurven steigen oder fallen nur
settings-monotonic-curves-tip = Kurveneditor: Ein Punkt oder Anfasser hält dort an, wo die Kurve umkehren würde, sodass sie die Null nur einmal kreuzt. Kurven, die bereits umkehren, lassen sich frei bearbeiten.
settings-curve-right-click-invert = Rechtsklick invertiert Kurven
settings-curve-right-click-invert-tip = Kurveneditor: Ein Rechtsklick auf eine freie Fläche invertiert die Kurve. Die Schaltfläche „Invertieren“ funktioniert immer.
settings-warn-gremlin = Warnen, wenn Speichern das Profilformat ändert
settings-warn-gremlin-tip = Profile mit Modifikatoren oder Ebenen werden in einem Format gespeichert, das Joystick Gremlin nicht öffnen kann.
gremlin-format-title = Joystick Gremlin wird es nicht öffnen
gremlin-format-text = { $file } nutzt Funktionen, die nur Joystick Gobloon hat (Modifikatoren und Ebenen). Nach dem Speichern kann Joystick Gremlin es nicht mehr öffnen.
gremlin-format-dont-ask = Nicht mehr fragen
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
edit-rename = Zum Umbenennen klicken
edit-description-hint = Klicken, um eine Notiz zu schreiben
edit-click-to-edit = Zum Bearbeiten klicken

# Files and modes
action-new = Neues Profil
wizard-open = Neues Profil mit dem Assistenten: jeden Controller einem virtuellen Joystick zuordnen
wizard-title = Assistent für neue Profile
wizard-name = Profilname
wizard-name-hint = z. B. Mein Flug-Setup
wizard-create = Erstellen
wizard-devices = Wähle für jeden Controller den virtuellen Joystick. Jede Achse, Taste und jeder Hat bekommt ein Map to vJoy.
wizard-none = Nicht zugeordnet
wizard-taken = Diesem virtuellen Joystick ist schon ein anderer Controller zugeordnet
wizard-no-vjoy = Noch keine virtuellen Joysticks: füge sie zuerst auf der Seite „Virtuelle Joysticks“ hinzu
wizard-fewer = { $vjoy } hat weniger Eingänge als { $device }: { $missing } bleiben frei
wizard-axes = { $n ->
    [one] 1 Achse
   *[other] { $n } Achsen
}
wizard-buttons = { $n ->
    [one] 1 Taste
   *[other] { $n } Tasten
}
wizard-hats = { $n ->
    [one] 1 Hat
   *[other] { $n } Hats
}
status-wizard = Profil erstellt: { $name }
action-save-as = Speichern unter…
status-new = Neues Profil
unsaved-title = Ungespeicherte Änderungen
unsaved-text = Änderungen an { $file } speichern?
unsaved-save = Speichern
unsaved-discard = Nicht speichern
cancel = Abbrechen
modes-edit = Modi bearbeiten
modifiers-edit = Modifikatoren
modifiers-none = Noch keine Modifikatoren. Ein Modifikator ist eine Taste, die die anderen Eingaben, solange sie gehalten wird, auf eine andere Ebene von Aktionen umschaltet.
modifiers-add = Modifikator hinzufügen
modifiers-press = Taste an einem Controller drücken…
modifiers-assign = Klicken, dann die Taste drücken, die dieser Modifikator verwenden soll
modifiers-remove = Modifikator und die Aktionen seiner Ebenen entfernen
modifier-input = Modifikator { $name }: Solange er gehalten wird, nutzen die anderen Eingaben seine Ebene; selbst führt er keine Aktionen aus
layer-base = Basis
layer-set = Diese Ebene hat Aktionen
layer-empty = Keine Aktionen auf dieser Ebene
layer-additive = Zur Basis hinzufügen
layer-additive-tip = An: Diese Aktionen laufen zusammen mit der Basisebene. Aus: Sie ersetzen sie, solange die Modifikatoren gehalten werden.
layer-keep-value = Letzten Wert behalten
layer-keep-value-tip = An: Endet diese Ebene, behalten ihre Achsen den letzten Wert. Aus: Sie kehren in die Ruhestellung zurück.
layers-tip = Ebenen: { $layers }
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
edit-curve-help = Punkt oder Griff ziehen, um ihn zu verschieben · Doppelklick fügt einen Punkt hinzu · Rechtsklick auf einen Punkt entfernt ihn
edit-curve-help-invert = Rechtsklick auf freie Fläche invertiert

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
vjoy-quickmap = Ausgang wählen
output-profile = Profil: { $name }
output-free = { $output }: frei
output-used = { $output }: { $n ->
    [one] von 1 Eingang genutzt
   *[other] von { $n } Eingängen genutzt
}
output-conflict = Gleichzeitig aktive Eingänge schreiben ihn
output-more = …und { $n } weitere
output-legend-used = Belegt
output-legend-conflict = Konflikt
output-legend-tip = Diese Elemente zeigen (das Profil ändert sich nicht)
output-same-input = Eine andere Aktion dieses Eingangs schreibt ihn bereits
output-same-input-clash = Eine andere Aktion dieses Eingangs schreibt denselben Ausgang: wähle einen anderen
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
status-latency = Latenz vom Lesen eines Controllers bis zur virtuellen Ausgabe, letzte Sekunde: im Mittel { $average } ms, maximal { $max } ms. Die Verzögerung des Controllers selbst ist nicht enthalten.

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
edit-merge-full = Sie führt bereits zwei andere Achsen zusammen; ändere eine davon in ihrem Editor.
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
edit-tts-front = Als Nächstes einreihen (Sprachausgabe des Systems: wie hinter allem anderen)
edit-tts-interrupt = Laufende Ansage unterbrechen
edit-preview = Vorschau

kind-light-led = LED-Beleuchtung
edit-led-device = Joystick mit LEDs
edit-led-no-device = Dieser Joystick ist nicht verbunden oder seine LEDs lassen sich nicht steuern
edit-led-id = LED
led-vkb-grip = RGB oben am Griff
led-vkb-hat = Kleine rote LED am Coolie-Hat
edit-led-color = Farbe

edit-led-color2 = Zweite Farbe
edit-led-blend = Farben
led-blend-color1 = Eine Farbe
led-blend-color2 = Nur zweite Farbe
led-blend-alternate = Farben abwechseln
led-blend-alternate-reverse = Abwechseln, zweite zuerst
led-blend-mix = Farben mischen
led-vkb-base-blue = Basis, blau (rechts)
led-vkb-base-red = Basis, rot (links)
edit-led-mode = Leuchtart
led-mode-on = An
led-mode-slow-blink = Langsam blinken
led-mode-fast-blink = Schnell blinken
led-mode-fastest-blink = Sehr schnell blinken
led-mode-off = Aus
edit-led-test = Jetzt einschalten
edit-led-on-activate = Beim Aktivieren des Profils oder Modus
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
settings-tray-double-click = Doppelklick aktiviert das Profil
settings-tray-title = Symbol im Infobereich
settings-autostart = Bei der Anmeldung starten (minimiert)
settings-activate-on-start = Profil beim Start aktivieren
tray-show = Joystick Gobloon anzeigen
tray-hiding = Geräte verbergen
tray-activate = „{ $name }“ aktivieren
tray-activate-none = Profil aktivieren
tray-quit = Beenden
autostart-error = Der Start bei der Anmeldung konnte nicht geändert werden: { $error }

# About
about-alpha = Alpha-Version
about-created = Erstellt von { $name }
about-support = Entwicklung unterstützen
about-support-tip = Über PayPal beitragen
about-source = Quellcode
about-issues = Problem melden
about-releases = Versionen
about-hiding = Geräte ausblenden: { $backend }
about-virtual = Virtuelle Joysticks: { $backend }
about-not-installed = nicht installiert
about-gremlin = Profile sind mit Joystick Gremlin kompatibel.
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
vjoy-name = Name
vjoy-name-suggest = Namen vorschlagen: nach dem Controller, den das Profil auf dieses Gerät legt, oder nach dem Layout
vjoy-name-invalid = Bis zu 79 Zeichen, ohne Leerzeichen am Anfang oder Ende, anders als die Namen der anderen Geräte.
vjoy-name-renamed = Spiele sehen ein umbenanntes Gerät als neues: Ihre Belegungen dafür müssen eventuell neu gesetzt werden.
vjoy-product = Produkt-ID
vjoy-product-tip = Daran unterscheiden Spiele die Geräte. Leer = nach der Gerätenummer gewählt; geben Sie die ID einer früheren Einrichtung ein, um die Belegungen eines Spiels zu behalten.
vjoy-product-invalid = Eine Hexadezimalzahl von BE01 bis BEFF (außer BEAD), die kein anderes Gerät verwendet.
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
curve-monotone = Glatt, ohne Überschwingen
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
calibration-jitter = Zitterfilter
calibration-jitter-tip = Ignoriert Änderungen kleiner als dieser Anteil des Wegs, für abgenutzte Achsen, die zittern. Keine Verzögerung; Enden und Mitte werden immer erreicht. Der Strich auf dem Balken zeigt, was gesendet wird. Zum Anwenden speichern
calibration-debounce = Entprellen
calibration-debounce-tip = Tasten und Hats dieses Geräts: nach einem Drücken oder Loslassen werden weitere Änderungen in dieser Zeit ignoriert, für prellende Schalter. Die erste Änderung kommt sofort durch
calibration-in-window = Die Kalibrierung ist in einem eigenen Fenster geöffnet.

# Device info
device-info = Geräteinformationen

device-leds = LEDs steuerbar (Aktion LED-Beleuchtung)
device-info-name = Gerätename
device-info-axes = Achsen
device-info-buttons = Tasten
device-info-hats = Coolie-Hats
device-info-vid = VID
device-info-pid = PID
device-info-joystick-id = Joystick-ID
device-info-guid = Geräte-GUID
device-info-copy = In die Zwischenablage kopieren

# Virtual gamepads
kind-map-to-gamepad = Auf Gamepad abbilden
gamepad-n = Gamepad { $n }
gamepad-tip = Virtuelles Gamepad im Xbox-Stil: Spiele, Steam und Proton sehen einen Xbox-360-Controller.
gamepad-add = Virtuelles Gamepad hinzufügen
gamepad-add-max = Höchstens { $n } Gamepads.
gamepad-add-n = Gamepad { $n } hinzufügen
gamepad-delete-n = Gamepad { $n } löschen
gamepad-missing = Gamepad { $n } gibt es noch nicht: Fügen Sie es auf der Seite Virtuelle Joysticks hinzu.
pad-a = A
pad-b = B
pad-x = X
pad-y = Y
pad-lb = LB
pad-rb = RB
pad-back = Back
pad-start = Start
pad-guide = Guide
pad-ls = Linken Stick drücken
pad-rs = Rechten Stick drücken
pad-dpad-up = Steuerkreuz oben
pad-dpad-down = Steuerkreuz unten
pad-dpad-left = Steuerkreuz links
pad-dpad-right = Steuerkreuz rechts
pad-left-x = Linker Stick X
pad-left-y = Linker Stick Y
pad-right-x = Rechter Stick X
pad-right-y = Rechter Stick Y
pad-lt = LT (linker Trigger)
pad-rt = RT (rechter Trigger)
pad-ls-short = LS
pad-rs-short = RS
pad-dpad = Steuerkreuz

system-banner-install = SystemGobloon installieren: Ausgeblendete Sticks sind für jedes Programm ausgeblendet, und virtuelle Geräte bleiben, solange Spiele laufen – wie mit HidHide und vJoy unter Windows.
system-banner-stopped = SystemGobloon ist installiert, läuft aber nicht.
system-banner-update = SystemGobloon { $version } ist installiert; dieses Programm bringt eine andere Version mit.
system-install = Installieren
system-update = Aktualisieren
system-start = Starten
system-remove = Entfernen
system-not-now = Nicht jetzt
system-admin = Fragt nach dem Administratorpasswort
system-about = Blendet Sticks für alle Programme aus und hält die virtuellen Joysticks, Gamepads, Tastatur und Maus, solange Spiele laufen. Nach der Installation ist kein Passwort mehr nötig.
system-status-missing = Nicht installiert.
system-status-stopped = Installiert, läuft nicht.
system-status-outdated = Version { $version } läuft: Aktualisieren Sie sie auf die Version dieses Programms.
system-status-current = Version { $version } läuft.
system-remove-title = SystemGobloon entfernen?
system-remove-text = Ausgeblendete Sticks sind dann nur ausgeblendet, solange Joystick Gobloon läuft, und virtuelle Geräte gibt es nur, solange ein Profil aktiv ist. Die ausgeblendeten Geräte und die Layouts der virtuellen Geräte bleiben erhalten.
system-installed = SystemGobloon ist installiert.
system-removed = SystemGobloon ist entfernt.
system-error = SystemGobloon: { $error }
vjoy-not-persistent = Diese Geräte gibt es nur, solange ein Profil aktiv ist oder ein Test sie hält. SystemGobloon (Einstellungen, Allgemein) hält sie ständig bereit.
hidhide-while-running = Geräte sind nur ausgeblendet, solange Joystick Gobloon läuft. SystemGobloon (Einstellungen, Allgemein) blendet sie ständig für alle Programme aus.
