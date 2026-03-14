# FastGTrack - Developer Guide

This document provides a technical overview of the FastGTrack codebase. It's designed to help you quickly understand the application's architecture, key files, state management, and the interaction between the Rust backend and the Slint UI frontend.

## 🏗 Architecture Overview

FastGTrack uses a hybrid architecture:
- **Frontend (UI)**: Built with [Slint](https://slint.dev/) (`*.slint`), a declarative UI framework.
- **Backend (Logic/State)**: Built in **Rust** (`*.rs`), handling state management, timer updates, file I/O, and the local SQLite database.
- **Database**: Uses `rusqlite` to store exercises, templates, logs, and PRs locally.
- **Platform**: Desktops (Windows/Linux/Mac) and Android.

The core design principle is **one-way data flow**: 
1. The UI triggers a `callback` (e.g., `finish-workout()`).
2. Rust intercepts the callback, mutates the `db` or internal `AppState`.
3. Rust calls `refresh_*` functions which push the new data models into the Slint UI properties.

---

## 📂 File Structure

### Rust Backend (`/src`)

The core logic is divided into robust, distinct modules:

*   **`main.rs`**
    *   The entry point. Minimal file that calls `fastgtrack::run()`.
*   **`models.rs`**
    *   **Purpose**: Defines all the data structures (Structs and Enums) used throughout the app.
    *   **Key Types**: `Exercise`, `MuscleGroup`, `WorkoutTemplate`, `ActiveWorkout`, `PlannedSet`, `SessionSummary`.
    *   **Helpers**: Includes UI formatting functions (e.g., `format_duration`, `format_relative_day`).
*   **`db.rs`**
    *   **Purpose**: Handles all SQLite interactions.
    *   **Key Functions**: 
        *   `init_schema()`: Creates the database structure automatically.
        *   `save_template()`, `list_templates()`, `load_template()`: Manages custom workout regimes.
        *   `save_completed_workout()`: Commits a live session and logs exact reps/weights.
        *   `evaluate_prs()`: Advanced logic to check if a new set broke a past Personal Record.
        *   `stats_*()`: A suite of functions querying historical progression data.
*   **`lib.rs`**
    *   **Purpose**: The central nervous system and glue code.
    *   **State Management**: Owns the `AppState` struct (which holds `db`, `active_workout`, `template_draft`).
    *   **Callbacks**: The `wire_callbacks` function binds all Slint `callback` events to Rust closures.
    *   **Refresh Strategy**: Contains `refresh_ui()`, `refresh_planner()`, `refresh_workout()`, etc. These grab data from `AppState`/`db` and push it to Slint models (`VecModel`).

### Slint UI Frontend (`/ui`)

The UI is cleanly separated into specialized views and components:

*   **`app.slint`**
    *   **Purpose**: The root Window (`MainWindow`).
    *   **Contains**: The persistent bottom Navigation Bar, Top Safe Area setups, and the active session overlay logic.
    *   **Properties**: Defines all the `in-out properties` and `callbacks` that the Rust backend connects to.
*   **`types.slint`**
    *   **Purpose**: Defines the UI-specific versions of data models (`struct`). Rust maps its internal models into these Slint structs to render lists. Example: `SessionCard`, `WorkoutSetRow`.
*   **`theme.slint`**
    *   **Purpose**: The central design system. Holds the `UiVars` global component, defining all colors (ink, surface, accent), font sizes, paddings, and border widths.
*   **`components.slint`**
    *   **Purpose**: Reusable atoms and small UI widgets. Includes standard layout boxes (`Panel`), custom Buttons, TextInputs, Checkboxes, and the `NavItem`.
*   **`screens/`** (The Main Tabs)
    *   **`home-screen.slint`**: Greets the user, highlights the daily workout target, and displays recent history.
    *   **`exercise-screen.slint`**: A searchable dictionary of all database exercises.
    *   **`planner-screen.slint`**: The "Drafting Phase" interface for building, re-ordering, and modifying reusable workout routines.
    *   **`workout-screen.slint`**: The live execution mode. Shows the timer, allows logging reps/weights for active sets, and auto-calculates rest times.
    *   **`stats-screen.slint`**: Renders analytical widgets, top exercises, PR progressions, and a monthly heatmap calendar.

---

## 🔄 Core Workflows

### 1. Starting the Application
1. `main()` calls `lib.rs::run()`.
2. Rust connects to (or creates) `fastgtrack.db` via `db.rs`.
3. Rust creates `AppState` and instantiates the Slint `MainWindow`.
4. `wire_callbacks()` connects Slint UI events to Rust modifiers.
5. `refresh_ui()` fetches all initial data (Templates, Exercises, Schedule) and hydrates the Slint models.
6. The Slint event loop `ui.run()` takes over.

### 2. Live Session (Workout) Tracking
1. **Start**: User clicks "Start Session" -> Slint fires `start-workout(template_id)`.
2. **Build**: Rust loads the template, resolves exercises, and builds an `ActiveWorkout` struct in `AppState`.
3. **Display**: Rust sets `is-session-fullscreen: true` in Slint and calls `refresh_workout()`.
4. **Log Set**: User types "10 reps, 100kg" and clicks Complete -> Slint fires `complete-set(ex_idx, set_idx)`.
5. **Evaluate**: Rust updates the memory state. It evaluates if the 100kg is a PR via `db.evaluate_prs()`.
6. **Rest**: Rust triggers the internal rest timer based on the planned rest duration.
7. **Finish**: User hits Finish -> Slint fires `finish-workout()`. Rust pushes the whole session to `db.save_completed_workout()`, clearing the `ActiveWorkout` state and updating PRs permanently.

### 3. Creating a Template (Planner)
1. User enters Planner. `AppState` maintains an empty `TemplateDraft`.
2. User adds an exercise -> Slint calls `add-draft-exercise()`.
3. Rust modifies `AppState::template_draft` and calls `refresh_planner()`.
4. User clicks Save -> Slint fires `save-template()`.
5. Rust invokes `db.save_template()` grouping the main template configuration with its `template_exercises` and `planned_sets` relationships.

---

## 🛠 Adding New Features

Follow this pattern when adding a new interactive feature:

1. **Slint Types (`ui/types.slint`)**: Add a new `export struct` if you need to pass a new data array from Rust.
2. **Slint App Component (`ui/app.slint`)**: 
   * Add the `in property` to accept data.
   * Add a `callback` if the UI needs to trigger an action.
3. **Slint Screens (`ui/screens/*.slint`)**: Implement the visual layout using the new property/callback.
4. **Rust DB/Models (`db.rs` / `models.rs`)**: If the feature requires saving data, create the struct representation and the SQLite queries.
5. **Rust Logic (`lib.rs`)**: 
   * In `wire_callbacks`, handle the new callback.
   * In a specific `refresh_*` function, query your Database, map it into your Slint Struct, and push it up using `.set_your_new_property(...)`.
