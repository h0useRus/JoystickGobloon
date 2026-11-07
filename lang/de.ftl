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
status-restored = Ungespeicherte Änderungen an { $file } wiederhergestellt
status-switched = { $program } im Vordergrund: { $file } läuft
status-switch-failed = { $program } im Vordergrund: { $file } ließ sich nicht laden
switch-unsaved-title = Profilwechsel nach Programm
switch-unsaved-text = { $file } hat ungespeicherte Änderungen. Ein Wechsel zu den Profilen deiner Programme würde es schließen, daher bleibt der Wechsel aus, bis du speicherst.
switch-save = Aktuelles speichern
switch-without = Ohne Wechsel fortfahren
switch-held-title = Profilwechsel nach Programm pausiert
switch-held-text = { $program } ist im Vordergrund, aber { $file } hat ungespeicherte Änderungen. Sein Profil läuft, sobald sie gespeichert sind.
switch-save-continue = Speichern und wechseln
switch-stop = Wechsel beenden
switch-stopped-title = Profil gestoppt
switch-stopped-text = Die Geräte von { $file } haben sich während des Laufs geändert:
switch-device-disconnected = { $device } wurde getrennt
switch-device-connected = { $device } wurde verbunden
switch-device-replaced = { $device } wurde mit einer neuen ID wieder verbunden
switch-virtual-changed = Die virtuellen Joysticks oder Gamepads haben sich geändert und reichen dem Profil nicht mehr
switch-activate = Aktivieren
status-switch-paused = Profilwechsel nach Programm pausiert: ungespeicherte Änderungen
status-switch-stopped = Profil gestoppt: seine Geräte haben sich geändert
status-switch-off = Profilwechsel nach Programm ist bis zur nächsten Aktivierung aus
status-saved = { $file } gespeichert
status-last-missing = Letztes Profil nicht gefunden: { $file }
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
vjoy-no-devices = Noch keine virtuellen Sticks oder Gamepads: Füge oben eins hinzu.
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
settings-focus-input-tip = Wird eine Achse, Taste oder ein Hat benutzt, wählt die Seite „Geräte und Belegungen“ den Tab des Geräts und die Eingabe, scrollt dorthin und zeigt ihre Aktionen.
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
settings-group-log = Protokolldatei
settings-log-level = Protokollstufe
settings-log-level-tip = Was Joystick Gobloon in JoystickGobloon.log neben seinen Einstellungen schreibt.
settings-log-all = Alles
settings-log-all-tip = Alles, auch Debug-Meldungen: um einem Problem auf die Spur zu kommen.
settings-log-important = Wichtig
settings-log-important-tip = Warnungen, Fehler und bemerkenswerte Ereignisse: Profil geladen oder gestartet, Gerät wieder da, Update.
settings-log-critical = Kritisch
settings-log-critical-tip = Nur Warnungen und Fehler.
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
apps-blind-all = Sieht verborgene Geräte nicht: Hier ist ein verborgenes Gerät vor allen Programmen verborgen, und kein Programm kann ausgenommen werden. Geräte mit dem Auge auf ihrem Reiter auf der Seite „Geräte und Belegungen“ verbergen oder zeigen
apps-inverse = Anwendungsliste umkehren
apps-inverse-tip = Aufgeführte Programme sehen verborgene Geräte nicht; alle anderen schon
apps-remove = Aus der Liste entfernen
apps-add-running = Laufendes Programm hinzufügen
apps-add-running-none = Keine anderen Programme mit Fenster laufen
apps-profile-none = Kein Profil
apps-profile-choose = Datei wählen…
apps-profile-tip = Profil, das läuft, solange dieses Programm im Vordergrund ist (wenn ein Profil aktiv ist)
apps-mode-start = Startmodus
apps-mode-tip = Modus, in dem das Profil für dieses Programm startet
apps-own-no-profile = Joystick Gobloon wählt nie ein Profil für sich selbst
apps-running-tip = Sein Profil läuft gerade

# Engine
action-activate = Profil aktivieren
action-deactivate = Profil stoppen
title-active = Aktiv
title-inactive = Inaktiv
title-not-saved = nicht gespeichert
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
send-mode-tip = Senden: wie die Ausgabe der Eingabe folgt
send-hold = Halten
send-hold-tip = Gedrückt, solange die Eingabe gedrückt ist
send-press = Nur drücken
send-press-tip = Die Eingabe drückt sie, eine andere lässt sie los
send-release = Nur loslassen
send-release-tip = Die Eingabe lässt sie los
send-pulse = Impuls
send-pulse-tip = Ein Druck fester Länge, egal wie lange die Eingabe gehalten wird
send-repeat = Wiederholen
send-repeat-tip = Drückt immer wieder, solange die Eingabe gehalten wird
send-toggle = Umschalten
send-toggle-tip = Der erste Druck hält sie, der nächste lässt sie los
send-pulse-ms = Drucklänge
send-delay-ms = Erste Wiederholung nach
send-period-ms = Wiederholen alle
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
kind-map-to-vjoy = Auf virtuellen Stick abbilden
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
wizard-devices = Wähle für jeden Controller den virtuellen Joystick. Jede Achse, Taste und jeder Hat bekommt ein „Auf virtuellen Stick abbilden“.
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

# Axis or hat as a button
vb-as-button = Als Taste behandeln
vb-as-button-tip = An: diese Aktionen laufen als Taste, gedrückt solange die Achse im Bereich ist (oder der Coolie-Hat in eine der gewählten Richtungen zeigt), losgelassen, wenn sie ihn verlässt.
vb-lower = Untere Grenze des Bereichs
vb-upper = Obere Grenze des Bereichs
vb-anywhere = Drücken, egal von welcher Seite die Achse in den Bereich kommt
vb-from-below = Nur drücken, wenn die Achse von unten in den Bereich kommt
vb-from-above = Nur drücken, wenn die Achse von oben in den Bereich kommt
vb-add = Aktionen hinzufügen, die als Taste laufen (Achsenbereich oder Coolie-Hat-Richtungen)
vb-remove = Diese Aktionen entfernen
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
keys-keyboard = Tasten auf einer Tastatur wählen
keys-clear = Tasten löschen
key-play-pause = Wiedergabe/Pause
key-media-stop = Stopp
key-previous-track = Vorheriger Titel
key-next-track = Nächster Titel
key-mute = Stumm
key-volume-down = Leiser
key-volume-up = Lauter
key-browser-back = Browser zurück
key-browser-forward = Browser vorwärts
key-browser-refresh = Browser aktualisieren
key-browser-home = Browser-Startseite
key-browser-search = Browser-Suche
key-browser-favorites = Browser-Favoriten
key-mail = E-Mail
key-calculator = Rechner
key-my-computer = Arbeitsplatz
key-media-player = Mediaplayer
key-sleep = Energie sparen
key-wake = Aufwecken
key-power = Ein/Aus
key-browser-stop = Browser stoppen
key-ro = Ro
key-yen = Yen
key-muhenkan = Muhenkan
key-henkan = Henkan
key-kana = Kana
key-hanja = Hanja
key-han-yeong = Han/Yeong
key-num-equals = Num =
key-num-comma = Num ,
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
macro-step-vjoy-button = Taste des virtuellen Sticks
macro-step-vjoy-axis = Achse des virtuellen Sticks
macro-step-vjoy-hat = Coolie-Hat des virtuellen Sticks
macro-step-pad-button = Gamepad-Taste
macro-step-pad-axis = Gamepad-Stick oder -Trigger
macro-step-pad-dpad = Gamepad-Steuerkreuz
macro-step-run-action = Aktion ausführen
macro-run-pick = Aktion wählen
macro-run-missing = Aktion nicht gefunden
macro-run-none = Keine benannten Aktionen: Gib zuerst einer Aktion in ihrem Titel einen Namen
macro-run-tip = Führt die benannte Aktion wie ein kurzer Druck auf ihre Eingabe aus
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
about-gremlin = Importiert Joystick-Gremlin-Profile ab R14.
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
vjoy-add = Virtuellen Stick hinzufügen
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
vjoy-config-done = Virtuelle Geräte geändert.
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
voice-device = Sprache
voice-tip = Sprachbefehle: Sag einen Satz, während eine Push-to-Talk-Eingabe gehalten wird; jeder Befehl wirkt wie eine Taste
voice-command-n = Befehl { $n }
voice-no-button = Sprachbefehle werden nur gehört, solange eine Eingabe mit der Aktion „Spracheingabe“ gehalten wird. Wähle eine Joystick-Taste, eine Tastaturtaste oder eine Gamepad-Taste, gib ihr die Aktion „Spracheingabe“ und füge dann hier Befehle und ihre Sätze hinzu.
voice-button = Hört zu, solange gehalten: { $device } · { $input }
voice-button-go = Zu dieser Eingabe
voice-add = Befehl
voice-from-action = Sprachbefehl mit dem Namen dieser Aktion hinzufügen
voice-from-action-name-first = Gib dieser Aktion zuerst einen Namen: Er wird zur Phrase des Sprachbefehls
voice-from-action-go = Zu ihrem Sprachbefehl
voice-from-action-taken = Ein anderer Sprachbefehl sagt bereits „{ $phrase }“: Gib dieser Aktion einen anderen Namen
voice-empty = Noch keine Sprachbefehle: füge mit der Schaltfläche oben einen hinzu.
voice-remove = Diesen Befehl und seine Belegungen entfernen
voice-phrases = Sag einen dieser Sätze:
voice-phrase-remove = Diesen Satz entfernen
voice-phrase-add = Satz hinzufügen
voice-no-phrases = Noch keine Sätze: dieser Befehl kann nicht gehört werden.
voice-hold = Wie lange der Befehl seine Taste hält, nachdem er gehört wurde
kind-voice-input = Spracheingabe
voice-mic = Mikrofon
voice-mic-default = Standardmikrofon
voice-mic-missing = Dieses Mikrofon ist nicht angeschlossen: solange hört das Standardmikrofon zu.
voice-input-hint = Hört auf Sprachbefehle, solange diese Eingabe gehalten wird. Die Befehle stehen auf dem Reiter „Sprache“ der Geräteseite.
voice-no-model = Kein Sprachmodell für Sprachbefehle: lade es unter Einstellungen, Sprache herunter.
voice-failed = Sprachbefehle funktionieren nicht: { $error }
voice-listening = Hört zu…
voice-loading = Sprachmodell wird geladen…
voice-heard = Gehört „{ $text }“ ({ $confidence } %): { $command }
voice-heard-unknown = Gehört „{ $text }“ ({ $confidence } %): kein Befehl
voice-heard-nothing = Nichts gehört
status-voice = Sprachbefehle: halte die Push-to-Talk-Eingabe, um zu sprechen
settings-voice = Sprache
settings-voice-model = Sprachmodell
settings-voice-model-hint = Sprachbefehle werden auf diesem Computer erkannt: nichts, was du sagst, verlässt ihn. Jede Sprache braucht ihr Sprachmodell, das einmal heruntergeladen wird.
settings-voice-language = Sprache der Sprachbefehle
settings-voice-language-program = Wie das Programm
settings-voice-no-model = Für { $language } gibt es noch kein Sprachmodell: wähle eine andere Sprache für Sprachbefehle.
settings-voice-download = Sprachmodell herunterladen ({ $size } MB)
settings-voice-downloading = { $done } von { $total } MB
settings-voice-cancel = Download abbrechen
settings-voice-installed = Sprachmodell installiert ({ $size } MB)
settings-voice-remove = Sprachmodell entfernen
settings-voice-download-failed = Das Sprachmodell konnte nicht heruntergeladen werden: { $error }
settings-voice-no-library = Die Spracherkennungsbibliothek fehlt: installiere Joystick Gobloon neu.
settings-voice-microphone = Mikrofon
settings-voice-gain = Mikrofonlautstärke
settings-voice-gain-tip = Erhöhe sie für ein leises Mikrofon, senke sie, wenn der Pegel das Ende des Balkens erreicht
settings-voice-test = Mikrofon testen
settings-voice-test-hint = Sprich so laut, wie du Befehle geben wirst: der Balken sollte etwa die Mitte erreichen, nie das Ende.
settings-voice-clipping = Zu laut
settings-voice-keep-open = Mikrofon offen halten
settings-voice-keep-open-tip = An: die Mikrofone des Profils bleiben offen, solange es aktiv ist, damit die ersten Worte nach dem Drücken der Taste gehört werden (manche Headsets sind nach dem Öffnen ein bis zwei Sekunden stumm). Erkannt wird erst, während die Taste gehalten wird.
settings-voice-confidence = Mindestsicherheit
settings-voice-confidence-tip = Ein weniger sicher gehörter Befehl tut nichts: erhöhe den Wert, wenn andere Sprache Befehle auslöst, senke ihn, wenn Befehle oft nicht erkannt werden
settings-voice-next-start = Änderungen gelten ab der nächsten Aktivierung eines Profils.
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

system-banner-install = System Gobloon installieren: Ausgeblendete Sticks sind für jedes Programm ausgeblendet, und virtuelle Geräte bleiben, solange Spiele laufen – wie mit HidHide und vJoy unter Windows.
system-banner-stopped = System Gobloon ist installiert, läuft aber nicht.
system-banner-update = System Gobloon { $version } ist installiert; dieses Programm bringt eine andere Version mit.
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
system-remove-title = System Gobloon entfernen?
system-remove-text = Ausgeblendete Sticks sind dann nur ausgeblendet, solange Joystick Gobloon läuft, und virtuelle Geräte gibt es nur, solange ein Profil aktiv ist. Die ausgeblendeten Geräte und die Layouts der virtuellen Geräte bleiben erhalten.
system-installed = System Gobloon ist installiert.
system-removed = System Gobloon ist entfernt.
system-error = System Gobloon: { $error }
vjoy-not-persistent = Diese Geräte gibt es nur, solange ein Profil aktiv ist oder ein Test sie hält. System Gobloon (Einstellungen, Allgemein) hält sie ständig bereit.
hidhide-while-running = Geräte sind nur ausgeblendet, solange Joystick Gobloon läuft. System Gobloon (Einstellungen, Allgemein) blendet sie ständig für alle Programme aus.
kind-button-axis = Achse aus Tasten
bax-to-vjoy = Eine Achse eines virtuellen Joysticks
bax-to-gamepad = Ein Stick oder Trigger eines virtuellen Gamepads
bax-to-logical = Eine Achse des logischen Geräts
bax-decrease = Bewegt die Achse nach unten (der gebundene Eingang nach oben)
bax-decrease-show = Diesen Eingang zeigen
down-for = Bewegt die Achse von { $input } nach unten (Achse aus Tasten)
input-not-shown = Das Gerät dieses Eingangs ist nicht auf der Seite „Geräte und Belegungen“: schließen Sie es an
bax-decrease-press = Taste oder Knopf drücken…
bax-decrease-none = Kein Eingang bewegt sie nach unten
bax-decrease-listen = Zuhören: Knopf oder Taste drücken, der die Achse nach unten bewegt
bax-decrease-clear = Eingang entfernen, der sie nach unten bewegt
bax-motion-tip = Wie die Tasten die Achse bewegen
bax-set = Wert setzen
bax-set-tip = Ein Druck setzt die Achse auf einen Wert, der andere Eingang auf das Gegenteil
bax-velocity = Geschwindigkeit
bax-velocity-tip = Die Achse fährt, solange eine Taste gehalten wird
bax-stepped = Stufen
bax-stepped-tip = Ein Druck geht zur nächsten Position
bax-encoder = Drehgeber
bax-encoder-tip = Ein Druck bewegt einen Schritt, umso größer, je schneller die Drücke kommen
bax-smooth = Weich
bax-smooth-tip = Ein Druck verschiebt das Ziel um einen Schritt, die Achse folgt weich
bax-value = Wert
bax-return = Beim Loslassen zurück
bax-speed = Geschwindigkeit
bax-acceleration = Beschleunigung
bax-max-speed = Höchstgeschwindigkeit
bax-step = Schritt
bax-max-step = Größter Schritt
bax-fast-rate = Größter Schritt bei
bax-spring = Zurück in Ruhe nach
bax-response = Reaktionszeit
bax-positions-tip = Klicken fügt eine Position hinzu, Ziehen verschiebt, Rechtsklick entfernt
bax-even-count = Wie viele Positionen
bax-even = So viele Positionen gleichmäßig verteilen
bax-start = Startwert
bax-start-tip = Die Achse beim Start des Profils auf diesen Wert setzen; aus, bleibt sie, wo sie ist

## Scripts
nav-scripts = Skripte
scripts-empty = Noch keine Skripte. Ein Skript ist ein kleines Lua-Programm, das auf deine Eingaben reagiert: füge eines im Profil hinzu oder eine .lua-Datei, die du in deinem eigenen Editor bearbeitest.
scripts-new = Neues Skript
scripts-new-name = Skript { $n }
scripts-add-file = Datei hinzufügen
scripts-python = Python-Skript von Joystick Gremlin: behalten, nicht ausgeführt
scripts-python-help = Das ist ein Python-Skript aus Joystick Gremlin. Joystick Gobloon behält es im Profil, führt aber nur Lua-Skripte aus.
scripts-in-profile = Im Profil
scripts-remove = Skript entfernen
scripts-name = Name des Skripts
scripts-choose-file = Andere Datei wählen
scripts-open-editor = In deinem Editor öffnen
scripts-file-missing = Die Datei kann nicht gelesen werden: { $error }
scripts-error = Zeile { $line }: { $message }
scripts-variables = Variablen
scripts-no-variables = Dieses Skript hat keine Variablen.
scripts-needs-value = Braucht einen Wert
scripts-device = Gerät
alert-script-error = Skriptfehler
