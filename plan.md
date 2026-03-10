# Gym Planer — App Spezifikation

## Tech Stack

| Layer | Technologie |
|---|---|
| UI & Animationen | Slint (GPU via OpenGL/Vulkan) |
| Datenbank | rusqlite (SQLite, gebundled) |
| Bilder | resvg (SVG) + image crate (PNG) |
| Async & Serialisierung | tokio + serde |
| Notifications | Kotlin-Bridge (JNI) |

---

## Datenmodell

### Exercise (Übungsdatenbank)
```
Exercise
├── id
├── name
├── muscle_group        // Enum: UpperBack, LowerBack, Chest, Core, Arms, Legs, Shoulders, Glutes, Cardio
├── equipment           // Enum: Barbell, Dumbbell, Machine, Cable, Bodyweight, Resistance Band, None
├── description
├── images[]            // Pfade zu gespeicherten PNG/SVG Dateien
├── is_timed            // Bool: zeitbasierter Satz (Plank etc.)
└── is_bodyweight       // Bool
```

### Set (Satz-Typ)
```
Set
├── set_type            // Enum: Normal, ToFailure, Timed, Dropset, Warmup
├── reps                // Optional (null wenn ToFailure oder Timed)
├── duration_seconds    // Optional (nur bei Timed)
├── weight              // Optional
├── weight_type         // Enum: Kg, Lbs, Bodyweight, Assisted(f32), BW_Plus(f32)
└── rest_seconds        // Pause nach diesem Satz
```

### WorkoutTemplate (Vorlage)
```
WorkoutTemplate
├── id
├── name
├── created_at
├── assigned_days[]     // Vec<Weekday> — kann mehreren Tagen zugewiesen sein
└── template_exercises[]
    ├── exercise_id
    ├── order
    └── planned_sets[]  // Vec<Set> — Vorgabe, änderbar im Workout
```

### WorkoutSession (Durchgeführtes Workout)
```
WorkoutSession
├── id
├── template_id         // Welche Vorlage wurde verwendet
├── started_at
├── finished_at
├── duration_seconds    // Berechnete Gesamtdauer
└── session_sets[]
    ├── exercise_id
    ├── set_number
    ├── set_type
    ├── reps_actual     // Was wirklich gemacht wurde
    ├── weight_actual
    ├── weight_type
    ├── duration_actual
    ├── completed       // Bool: abgehakt
    └── is_pr           // Bool: persönlicher Rekord
```

### PersonalRecord
```
PersonalRecord
├── id
├── exercise_id
├── record_type         // Enum: MaxWeight, Max1RM, MaxReps, MaxDuration
├── value
├── achieved_at
└── session_id
```

---

## Features

### Übungsdatenbank
- Eigene Übungen anlegen mit Name, Muskelgruppe, Equipment, Beschreibung, Bilder
- Bilder pro Übung (PNG/SVG), werden sinnvoll im Workout-View angezeigt
- Vordefinierte Übungen als Starterpaket (aus free-exercise-db, Public Domain)
- Filterbar nach: Muskelgruppe, Equipment, eigene/alle
- Sortierbar nach: Häufigkeit, Gewichts-Wachstum (%), zuletzt trainiert

### Muskelgruppen
Upper Back, Lower Back, Chest, Core, Arms, Legs, Shoulders, Glutes, Cardio

### Workout Builder
1. Workout benennen
2. Wochentage zuweisen (Mehrfachauswahl möglich)
3. Übungen aus Datenbank hinzufügen (suchbar, filterbar)
4. Pro Übung: Sätze konfigurieren
   - Satz-Typ wählen (Normal / Bis Versagen / Zeitbasiert / Warmup)
   - Reps, Gewicht, Gewichtstyp vorbelegen
   - Pause nach dem Satz einstellen
5. Reihenfolge der Übungen per Drag & Drop ändern
6. Speichern als Template

### Workout Planer
- Wochenansicht mit zugewiesenen Workouts pro Tag
- Mehrere Workouts pro Tag möglich
- Workout auch ohne festen Tag starten ("freies Training")

### Workout View (Kernfeature)
```
┌─────────────────────────────┐
│  [Letzte Übung] ↑ scroll    │  ← ausgegraut/kleiner
├─────────────────────────────┤
│  AKTUELLE ÜBUNG             │
│  Bild der Übung             │
│                             │
│  Satz 1  [8 Reps / 80kg]   │  ← letzter Wert als Referenz
│  Satz 2  [8 Reps / 80kg]   │
│  Satz 3  [8 Reps / 80kg]   │
│                             │
│  [████████░░░░] 45s Pause  │  ← grüner Balken, abschaltbar
├─────────────────────────────┤
│  [Nächste Übung] ↓ scroll   │  ← ausgegraut/kleiner
└─────────────────────────────┘
```

**Satz abhaken:**
- Satz antippen → Reps/Gewicht bestätigen oder ändern
- Satz als erledigt markieren → Pause-Balken startet (wenn aktiviert)
- Grüner animierter Balken zeigt Restzeit der Pause
- Pause-Timer: ein/aus pro Workout einstellbar, Dauer aus Template

**Satz-Referenzwerte:**
- Jeder Satz zeigt die Werte der letzten Session für denselben Satz
- Neuer PR → visuelles Highlight (z.B. goldener Indikator)

**Übung wechseln:**
- Nach oben scrollen → vorherige Übung
- Nach unten scrollen → nächste Übung
- Swipe-basierte Navigation

**Bilder & Beschreibung:**
- Übungsbild oben in der Karte
- Beschreibung ausklappbar

**Workout beenden:**
- Zusammenfassung: Dauer, Volumen, PRs
- Dialog: Was soll am Template gespeichert werden?
  - [ ] Neue Sätze übernehmen
  - [ ] Neue Übungen übernehmen
  - [ ] Geänderte Reps-Zahl übernehmen
  - [ ] Geändertes Gewicht übernehmen
- Bestätigen → Session wird gespeichert

### Notifications (via Kotlin-Bridge)
- Aktiver Satz direkt aus der Notification abhaken
- Pause-Timer in der Notification sichtbar
- Buttons: "Satz erledigt" / "Überspringen" / "Workout beenden"

### Einheiten
- Umschaltung zwischen **kg** und **lbs** global in den Einstellungen
- Alle gespeicherten Werte intern in kg, Anzeige konvertiert

### Start Page
- Letzten 5 Workouts mit Datum und "vor X Tagen"
- Schnellstart: nächstes geplantes Workout des Tages
- Wochenübersicht: welche Tage wurde trainiert
- Motivations-Streak: X Tage in Folge trainiert

### Statistik-Page
**Kalender:**
- Monatliche Kalenderansicht
- Trainingstage farblich markiert
- PRs mit eigenem Symbol im Kalender

**Workout-Statistiken:**
- Wie oft jedes Template trainiert wurde
- Durchschnittliche Dauer pro Workout
- Volumen-Verlauf (Gesamtgewicht pro Session)

### Übungsstatistik
- Gewichtsverlauf als Liniendiagramm
- 1RM-Verlauf (berechnet aus Reps + Gewicht via Epley-Formel)
- Kalender: wann diese Übung trainiert wurde
- PRs mit Datum aufgelistet
- Sortieroptionen:
  - Höchstes Gewichtswachstum in %
  - Häufigsten trainiert
  - Zuletzt trainiert
  - Aktuelle Bestleistung

### Personal Records (PRs)
- Automatisch erkannt bei jeder Session
- Typen: Max Gewicht, Max 1RM, Max Reps, Max Dauer (bei Timed)
- In Workout-View hervorgehoben wenn gesetzt
- Chronologisch in der Übungsstatistik
- Im Kalender mit eigenem Marker

---

## Satz-Typen im Detail

| Typ | Reps | Gewicht | Zeit | Beschreibung |
|---|---|---|---|---|
| Normal | ✅ | ✅ | ❌ | Standard Satz |
| Bis Versagen | ❌ | ✅ | ❌ | Reps werden erst nach dem Satz eingetragen |
| Zeitbasiert | ❌ | optional | ✅ | z.B. Plank 60s |
| Bodyweight | ✅ | ❌ | ❌ | Nur Körpergewicht |
| BW + Zusatz | ✅ | ✅ (positiv) | ❌ | z.B. Weighted Pullup |
| Assisted | ✅ | ✅ (negativ) | ❌ | z.B. Klimmzug-Maschine |
| Warmup | ✅ | ✅ | ❌ | Zählt nicht für Statistik/PR |
| Dropset | ✅ | ✅ | ❌ | Mehrere Gewichte in Folge |

---

## Supersets & Dropsets

### Supersets
Zwei oder mehr Übungen werden abwechselnd ohne Pause dazwischen ausgeführt.

**Datenmodell-Erweiterung:**
```
WorkoutTemplateExercise
├── ...
├── superset_group_id   // Optional — gleiche ID = gehören zusammen
└── superset_order      // Reihenfolge innerhalb der Gruppe
```

**Workout-View bei Supersets:**
```
┌─────────────────────────────┐
│  SUPERSET  [A / B]          │  ← Tab-Umschaltung
│                             │
│  A: Bench Press             │
│  Satz 1  [10 / 80kg] ✓     │
│  Satz 2  [10 / 80kg]        │
│                             │
│  → Dann direkt: Rows        │
│  B: Barbell Row             │
│  Satz 1  [10 / 70kg]        │
└─────────────────────────────┘
```
- Kein Pause-Timer zwischen A und B
- Pause-Timer läuft erst nach dem letzten B-Satz
- Im Workout-View klar als "Superset" gekennzeichnet

### Dropsets
Mehrere Gewichte in direkter Folge, kein Pause dazwischen.

**Datenmodell-Erweiterung:**
```
Set
├── ...
├── is_dropset          // Bool
└── dropset_index       // 0 = erster Drop, 1 = zweiter usw.
```

**Workout-View bei Dropsets:**
```
Satz 3  80kg → 60kg → 40kg   [Dropset]
         ✓       ✓      □
```
- Jeder Drop wird einzeln abgehakt
- Kein Pause-Timer zwischen den Drops
- Pause erst nach dem letzten Drop

---

## Export & Import

### Export-Formate
| Format | Inhalt | Verwendung |
|---|---|---|
| `.gymplan` (JSON) | Alles: Sessions, Templates, Übungen, PRs | Vollbackup / Geräte-Wechsel |
| `.json` | Nur Templates + Übungen | Workout-Sharing |
| `.csv` | Session-History | Analyse in Excel/Sheets |

### Export-Optionen
- **Vollbackup** — alles inklusive History und PRs
- **Nur Templates** — Workout-Vorlagen teilen
- **Nur History** — Trainingslog als CSV
- **Zeitraum-Filter** — z.B. nur letztes Jahr exportieren

### Import
- `.gymplan` Datei importieren → Merge-Dialog
  - Duplikate erkennen (gleicher Übungsname)
  - Wählen: überschreiben / zusammenführen / überspringen
- Templates importieren (von anderem User geteilt)
- CSV-Import für Migration von anderen Apps (MyFitnessPal, Strong etc.)

### Speicherort
- Export landet im Android Downloads-Ordner
- Sharing via Android Share-Sheet (WhatsApp, Google Drive, etc.)
- Auto-Backup: optional wöchentlich automatisch in einen konfigurierbaren Ordner

---

## Home Screen Widget

### Implementierung
Widgets auf Android = **RemoteViews** (Kotlin-Bridge zwingend nötig) — kein Rust-UI möglich hier. Rust liefert die Daten, Kotlin rendert das Widget.

### Widget-Typen

**Small Widget (2×2)**
```
┌──────────────┐
│ 💪 Chest Day │
│ Heute geplant│
│  [Starten]   │
└──────────────┘
```

**Medium Widget (4×2)**
```
┌────────────────────────────┐
│ Aktives Workout: Push Day  │
│ Bench Press — Satz 2/4     │
│ [████████░░] 32s Pause     │
│ [Satz ✓]    [Überspringen] │
└────────────────────────────┘
```

**Large Widget (4×4)**
```
┌────────────────────────────┐
│ Diese Woche                │
│ Mo ✓  Di ✓  Mi -  Do ?    │
│                            │
│ Nächstes Workout:          │
│ Pull Day — heute           │
│ Letztes: vor 2 Tagen       │
│      [Starten]             │
└────────────────────────────┘
```

### Widget-States
- **Kein Workout aktiv** → nächstes geplantes Workout + Starten-Button
- **Workout aktiv** → aktuelle Übung, Satz, Pause-Timer
- **Pause läuft** → Countdown + "Satz erledigt" Button
- **Kein Workout heute** → letztes Workout + Streak

---

## Offene Punkte / Noch zu entscheiden

- [ ] Körpergewicht-Tracking für BW-Übungen (optional separat loggbar)?
- [ ] Reminder-Notifications wenn kein Workout an geplantem Tag?
- [ ] Dark Mode only oder auch Light Mode?
- [ ] Übungsdatenbank: lokal only — oder QR-Code/Link Sharing von Templates?

---

## MVP Reihenfolge

**Phase 1 — Core**
1. DB-Schema + rusqlite Setup
2. Übungen anlegen (CRUD)
3. Workout-Template Builder
4. Workout-View (Kernfeature)
5. Session speichern

**Phase 2 — Statistik**
6. Start Page mit History
7. Kalender-View
8. Übungsstatistik + Gewichtsverlauf
9. PR-Tracking

**Phase 3 — Polish**
10. Notifications via Kotlin-Bridge
11. Bilder pro Übung
12. Pause-Timer Balken Animationen
13. Workout-Ende Dialog (Template-Update)
14. kg/lbs Umschaltung
15. Supersets & Dropsets
16. Export & Import (.gymplan, CSV)
17. Home Screen Widget (Kotlin-Bridge)
18.
