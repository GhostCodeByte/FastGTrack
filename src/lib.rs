mod db;
mod models;
mod settings;

use std::{cell::RefCell, fs, path::PathBuf, rc::Rc};

use anyhow::{Context, Result};
use chrono::{Datelike, Local, NaiveDate};
use db::Database;
use models::{
    ActiveExercise, ActiveSet, ActiveWorkout, AppExportBundle, SetType, TemplateDraft,
    TemplateDraftExercise, WeightType, format_duration, format_relative_day, format_set_actual,
    format_set_plan, normalize_days,
};
use settings::{
    AppSettings, app_storage_dir, backups_dir, ensure_storage_dirs, exports_dir, latest_json_file,
    load_settings, normalize_hex_color, save_settings, settings_path, timestamped_file,
};
#[cfg(target_os = "android")]
use settings::set_app_storage_dir;
use slint::{ModelRc, SharedString, VecModel};

slint::include_modules!();

/// Holds the central, active state of the FastGTrack application.
struct AppState {
    db: Rc<Database>,
    database_path: PathBuf,
    settings: AppSettings,
    template_draft: TemplateDraft,
    active_workout: Option<ActiveWorkout>,
    selected_exercise_index: usize,
    status_message: String,
    last_export_label: String,
    last_backup_label: String,
}

impl AppState {
    /// Creates a new, initially empty application state wrapping the given database connection.
    fn new(db: Rc<Database>, database_path: PathBuf, settings: AppSettings) -> Self {
        Self {
            db,
            database_path,
            settings,
            template_draft: TemplateDraft::default(),
            active_workout: None,
            selected_exercise_index: 0,
            status_message: "".into(),
            last_export_label: "No export yet".into(),
            last_backup_label: "No backup yet".into(),
        }
    }
}

/// Starts the FastGTrack application using the default database path.
pub fn run() -> Result<()> {
    run_with_database_path(default_database_path())
}

/// Handles the core initialization logic: setting up the database, state, UI, and event system.
fn run_with_database_path(database_path: PathBuf) -> Result<()> {
    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent).context("failed to create app data directory")?;
    }

    ensure_storage_dirs()?;
    let settings = load_settings().context("failed to load app settings")?;
    let db = Rc::new(Database::open(&database_path).context("failed to bootstrap FastGTrack DB")?);
    let state = Rc::new(RefCell::new(AppState::new(
        db,
        database_path.clone(),
        settings,
    )));
    let ui = MainWindow::new().context("failed to construct main window")?;
    apply_theme(&ui, &state.borrow().settings);

    wire_callbacks(&ui, state.clone());
    refresh_ui(&ui, &mut state.borrow_mut())?;
    ui.run().context("slint runtime error")?;
    Ok(())
}

#[cfg(not(target_os = "android"))]
/// Returns the default fallback location for the local database on desktop environments.
fn default_database_path() -> PathBuf {
    app_storage_dir().join("fastgtrack.db")
}

#[cfg(target_os = "android")]
/// Returns the default path for the database on Android structures.
fn default_database_path() -> PathBuf {
    PathBuf::from("/data/local/tmp/fastgtrack.db")
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    if let Err(error) = slint::android::init(app.clone()) {
        eprintln!("failed to init Slint Android backend: {error}");
        return;
    }

    let storage_dir = app
        .internal_data_path()
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp"))
        .join("FastGTrack");
    set_app_storage_dir(&storage_dir);

    let database_path = storage_dir.join("fastgtrack.db");

    if let Err(error) = run_with_database_path(database_path) {
        eprintln!("FastGTrack Android startup failed: {error}");
    }
}

fn apply_theme(ui: &MainWindow, settings: &AppSettings) {
    let colors = ui.global::<UiVars>();
    let background = parse_color_or_default(
        &settings.background_color,
        slint::Color::from_rgb_u8(0xEF, 0xED, 0xE8),
    );
    let accent = parse_color_or_default(
        &settings.accent_color,
        slint::Color::from_rgb_u8(0xF2, 0xCF, 0x00),
    );
    let text = parse_color_or_default(
        &settings.text_color,
        slint::Color::from_rgb_u8(0x11, 0x11, 0x11),
    );
    let soft = blend(text, background, 0.45);
    let muted = blend(text, background, 0.62);

    colors.set_background(background);
    colors.set_surface(background);
    colors.set_surface_strong(blend(background, text, 0.06));
    colors.set_surface_soft(blend(background, text, 0.03));
    colors.set_foreground(text);
    colors.set_foreground_soft(soft);
    colors.set_foreground_muted(muted);
    colors.set_accent(accent);
    colors.set_accent_strong(blend(accent, text, 0.18));
    colors.set_danger(slint::Color::from_rgb_u8(0x8E, 0x24, 0x24));
    colors.set_border_soft(soft);
    colors.set_border_strong(text);
    colors.set_nav_bg(background);
    colors.set_nav_surface(blend(background, text, 0.02));
    colors.set_nav_divider(text);
    colors.set_nav_active_bg(accent);
    colors.set_nav_active_ink(text);
    colors.set_nav_inactive_bg(blend(background, text, 0.03));
    colors.set_nav_inactive_ink(muted);
    colors.set_nav_inactive_stroke(soft);
    colors.set_screen_bg(background);
    colors.set_panel_bg(background);
    colors.set_panel_bg_strong(blend(background, text, 0.06));
    colors.set_panel_bg_soft(blend(background, text, 0.03));
    colors.set_ink(text);
    colors.set_ink_soft(soft);
    colors.set_ink_muted(muted);
    colors.set_accent_deep(blend(accent, text, 0.18));
    colors.set_quiet_stroke(soft);
}

fn parse_color_or_default(input: &str, default: slint::Color) -> slint::Color {
    let normalized = normalize_hex_color(input, "#000000");
    let hex = normalized.trim_start_matches('#');
    if hex.len() != 6 {
        return default;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok();
    let g = u8::from_str_radix(&hex[2..4], 16).ok();
    let b = u8::from_str_radix(&hex[4..6], 16).ok();
    match (r, g, b) {
        (Some(r), Some(g), Some(b)) => slint::Color::from_rgb_u8(r, g, b),
        _ => default,
    }
}

fn blend(a: slint::Color, b: slint::Color, amount: f32) -> slint::Color {
    let mix = |x: u8, y: u8| ((x as f32 * (1.0 - amount)) + (y as f32 * amount)).round() as u8;
    slint::Color::from_rgb_u8(
        mix(a.red(), b.red()),
        mix(a.green(), b.green()),
        mix(a.blue(), b.blue()),
    )
}

fn sync_settings_ui(ui: &MainWindow, state: &AppState) {
    ui.set_settings_accent_color(state.settings.accent_color.clone().into());
    ui.set_settings_background_color(state.settings.background_color.clone().into());
    ui.set_settings_text_color(state.settings.text_color.clone().into());
    ui.set_settings_default_rest(state.settings.default_rest_seconds.to_string().into());
    ui.set_settings_weight_unit(state.settings.default_weight_unit.clone().into());
    ui.set_settings_set_count(state.settings.default_set_count.to_string().into());
    ui.set_settings_rep_target(state.settings.default_rep_target.to_string().into());
    ui.set_settings_storage_path(app_storage_dir().display().to_string().into());
    ui.set_settings_database_path(state.database_path.display().to_string().into());
    ui.set_settings_settings_path(settings_path().display().to_string().into());
    ui.set_settings_last_export(state.last_export_label.clone().into());
    ui.set_settings_last_backup(state.last_backup_label.clone().into());
    ui.set_settings_app_version(env!("CARGO_PKG_VERSION").into());
    ui.set_settings_schema_version("1".into());
}

fn save_current_settings(ui: &MainWindow, state: &mut AppState) -> Result<()> {
    state.settings.accent_color = normalize_hex_color(
        &ui.get_settings_accent_color().to_string(),
        &state.settings.accent_color,
    );
    state.settings.background_color = normalize_hex_color(
        &ui.get_settings_background_color().to_string(),
        &state.settings.background_color,
    );
    state.settings.text_color = normalize_hex_color(
        &ui.get_settings_text_color().to_string(),
        &state.settings.text_color,
    );
    state.settings.default_rest_seconds = parse_i32(
        &ui.get_settings_default_rest().to_string(),
        state.settings.default_rest_seconds,
    )?;
    state.settings.default_set_count = parse_i32(
        &ui.get_settings_set_count().to_string(),
        state.settings.default_set_count,
    )?;
    state.settings.default_rep_target = parse_i32(
        &ui.get_settings_rep_target().to_string(),
        state.settings.default_rep_target,
    )?;
    state.settings.default_weight_unit = ui.get_settings_weight_unit().to_string();
    save_settings(&state.settings)?;
    apply_theme(ui, &state.settings);
    sync_settings_ui(ui, state);
    Ok(())
}

fn write_bundle(path: &PathBuf, bundle: &AppExportBundle) -> Result<()> {
    let json = serde_json::to_string_pretty(bundle)?;
    fs::write(path, json)?;
    Ok(())
}

fn read_bundle(path: &PathBuf) -> Result<AppExportBundle> {
    let json = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

/// Connects GUI events triggered from Slint with their corresponding logic updates in `AppState`.
fn wire_callbacks(ui: &MainWindow, state: Rc<RefCell<AppState>>) {
    // Search: no DB reads, operates purely on the in-memory exercise cache.
    let weak = ui.as_weak();
    let state_for_search = state.clone();
    ui.on_search_query_changed(move || {
        if let Some(ui) = weak.upgrade() {
            refresh_exercises(&ui, &state_for_search.borrow());
        }
    });

    let weak = ui.as_weak();
    let state_for_save_settings = state.clone();
    ui.on_save_settings(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_save_settings.borrow_mut();
            if let Some(ui) = weak.upgrade() {
                save_current_settings(&ui, &mut state)?;
                state.status_message = "Settings saved.".into();
            }
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_save_settings, outcome, |ui, state| {
            sync_settings_ui(ui, state);
            refresh_home(ui, state).ok();
            refresh_planner(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_reset_colors = state.clone();
    ui.on_reset_colors(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_reset_colors.borrow_mut();
            state.settings.accent_color = AppSettings::default().accent_color;
            state.settings.background_color = AppSettings::default().background_color;
            state.settings.text_color = AppSettings::default().text_color;
            save_settings(&state.settings)?;
            if let Some(ui) = weak.upgrade() {
                apply_theme(&ui, &state.settings);
                sync_settings_ui(&ui, &state);
            }
            state.status_message = "Colors reset to defaults.".into();
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_reset_colors, outcome, |ui, state| {
            sync_settings_ui(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_reset_defaults = state.clone();
    ui.on_reset_defaults(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_reset_defaults.borrow_mut();
            let defaults = AppSettings::default();
            state.settings.default_rest_seconds = defaults.default_rest_seconds;
            state.settings.default_weight_unit = defaults.default_weight_unit;
            state.settings.default_set_count = defaults.default_set_count;
            state.settings.default_rep_target = defaults.default_rep_target;
            save_settings(&state.settings)?;
            if let Some(ui) = weak.upgrade() {
                sync_settings_ui(&ui, &state);
            }
            state.status_message = "Defaults reset.".into();
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_reset_defaults, outcome, |ui, state| {
            sync_settings_ui(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_export = state.clone();
    ui.on_export_all_data(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_export.borrow_mut();
            let path = timestamped_file(exports_dir(), "fastgtrack-export");
            let bundle = state.db.export_bundle(&state.settings)?;
            write_bundle(&path, &bundle)?;
            state.last_export_label = path.display().to_string();
            state.status_message = format!("Exported data to {}", path.display());
            if let Some(ui) = weak.upgrade() {
                sync_settings_ui(&ui, &state);
            }
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_export, outcome, |ui, state| {
            sync_settings_ui(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_import = state.clone();
    ui.on_import_latest_export(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_import.borrow_mut();
            let path = latest_json_file(exports_dir()).context("no export file found")?;
            let bundle = read_bundle(&path)?;
            state.db.import_bundle(&bundle)?;
            state.settings = bundle.settings.clone();
            save_settings(&state.settings)?;
            if let Some(ui) = weak.upgrade() {
                apply_theme(&ui, &state.settings);
                sync_settings_ui(&ui, &state);
                refresh_ui(&ui, &mut state)?;
            }
            state.last_export_label = path.display().to_string();
            state.status_message = format!("Imported data from {}", path.display());
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_import, outcome, |ui, state| {
            sync_settings_ui(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_backup = state.clone();
    ui.on_create_backup(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_backup.borrow_mut();
            let path = timestamped_file(backups_dir(), "fastgtrack-backup");
            let bundle = state.db.export_bundle(&state.settings)?;
            write_bundle(&path, &bundle)?;
            state.last_backup_label = path.display().to_string();
            state.status_message = format!("Backup created at {}", path.display());
            if let Some(ui) = weak.upgrade() {
                sync_settings_ui(&ui, &state);
            }
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_backup, outcome, |ui, state| {
            sync_settings_ui(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_restore = state.clone();
    ui.on_restore_latest_backup(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_restore.borrow_mut();
            let path = latest_json_file(backups_dir()).context("no backup file found")?;
            let bundle = read_bundle(&path)?;
            state.db.import_bundle(&bundle)?;
            state.settings = bundle.settings.clone();
            save_settings(&state.settings)?;
            if let Some(ui) = weak.upgrade() {
                apply_theme(&ui, &state.settings);
                sync_settings_ui(&ui, &state);
                refresh_ui(&ui, &mut state)?;
            }
            state.last_backup_label = path.display().to_string();
            state.status_message = format!("Backup restored from {}", path.display());
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_restore, outcome, |ui, state| {
            sync_settings_ui(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_clear_history = state.clone();
    ui.on_clear_history(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_clear_history.borrow_mut();
            state.db.clear_history()?;
            state.status_message = "Workout history deleted.".into();
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_clear_history, outcome, |ui, state| {
            refresh_home(ui, state).ok();
            refresh_stats(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_clear_templates = state.clone();
    ui.on_clear_templates(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_clear_templates.borrow_mut();
            state.db.clear_templates()?;
            state.template_draft = TemplateDraft::default();
            state.status_message = "Templates deleted.".into();
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_clear_templates, outcome, |ui, state| {
            refresh_home(ui, state).ok();
            refresh_planner(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_clear_custom_exercises = state.clone();
    ui.on_clear_custom_exercises(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_clear_custom_exercises.borrow_mut();
            state.db.clear_custom_exercises()?;
            state.status_message = "Custom exercises deleted.".into();
            Ok(())
        })();
        with_partial_refresh(
            &weak,
            &state_for_clear_custom_exercises,
            outcome,
            |ui, state| {
                refresh_exercises(ui, state);
                refresh_status(ui, state);
            },
        );
    });

    let weak = ui.as_weak();
    let state_for_reset_all = state.clone();
    ui.on_reset_all_data(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_reset_all.borrow_mut();
            state.db.clear_all_data()?;
            state.template_draft = TemplateDraft::default();
            state.active_workout = None;
            state.selected_exercise_index = 0;
            state.status_message = "All local data deleted.".into();
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_reset_all, outcome, |ui, state| {
            sync_settings_ui(ui, state);
            refresh_exercises(ui, state);
            refresh_home(ui, state).ok();
            refresh_planner(ui, state);
            refresh_workout(ui, state);
            refresh_stats(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_add = state.clone();
    ui.on_add_draft_exercise(
        move |exercise_id, set_type, sets_count, reps, duration, weight, rest, weight_type| {
            let outcome = (|| -> Result<()> {
                let mut state = state_for_add.borrow_mut();
                let exercise_id_i64 = i64::from(exercise_id);

                let ex = state
                    .db
                    .list_exercises()?
                    .into_iter()
                    .find(|e| e.id == exercise_id_i64)
                    .context("exercise not found")?;

                let sets_count = parse_i32(&sets_count, state.settings.default_set_count)?;
                let reps = parse_optional_i32(&reps)?;
                let duration = parse_optional_i32(&duration)?;
                let weight = parse_optional_f32(&weight)?;
                let rest_seconds = parse_i32(&rest, state.settings.default_rest_seconds)?;

                let mut sets = Vec::new();
                for _ in 0..sets_count {
                    sets.push(crate::models::DraftSet {
                        reps,
                        duration_seconds: duration,
                        weight,
                        rest_seconds,
                    });
                }

                state.template_draft.exercises.push(TemplateDraftExercise {
                    exercise_id: exercise_id_i64,
                    exercise_name: ex.name.clone(),
                    set_type: SetType::from_label(set_type.as_str()),
                    weight_type: parse_weight_type(weight_type.as_str()),
                    sets,
                });
                state.status_message = "".into();
                Ok(())
            })();
            // Adding to draft only changes the draft panel.
            with_partial_refresh(&weak, &state_for_add, outcome, |ui, state| {
                refresh_planner(ui, state);
                refresh_status(ui, state);
            });
        },
    );

    let weak = ui.as_weak();
    let state_for_remove_draft = state.clone();
    ui.on_remove_draft_exercise(move |index| {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_remove_draft.borrow_mut();
            let idx = usize::try_from(index).unwrap_or_default();
            if idx < state.template_draft.exercises.len() {
                state.template_draft.exercises.remove(idx);
            }
            state.status_message = "".into();
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_remove_draft, outcome, |ui, state| {
            refresh_planner(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_add_set = state.clone();
    ui.on_add_set(move |index| {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_add_set.borrow_mut();
            let idx = usize::try_from(index).unwrap_or_default();
            let default_rep_target = state.settings.default_rep_target;
            let default_rest_seconds = state.settings.default_rest_seconds;
            if let Some(draft) = state.template_draft.exercises.get_mut(idx) {
                let last_set = draft
                    .sets
                    .last()
                    .cloned()
                    .unwrap_or(crate::models::DraftSet {
                        reps: Some(default_rep_target),
                        duration_seconds: None,
                        weight: None,
                        rest_seconds: default_rest_seconds,
                    });
                draft.sets.push(last_set);
            }
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_add_set, outcome, |ui, state| {
            refresh_planner(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_remove_set = state.clone();
    ui.on_remove_set(move |index, set_index| {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_remove_set.borrow_mut();
            let idx = usize::try_from(index).unwrap_or_default();
            let set_idx = usize::try_from(set_index).unwrap_or_default();
            if let Some(draft) = state.template_draft.exercises.get_mut(idx) {
                if draft.sets.len() > 1 && set_idx < draft.sets.len() {
                    draft.sets.remove(set_idx);
                }
            }
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_remove_set, outcome, |ui, state| {
            refresh_planner(ui, state);
        });
    });

    let _weak = ui.as_weak();
    let state_for_update_set = state.clone();
    ui.on_update_set(move |index, set_index, reps, weight, rest| {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_update_set.borrow_mut();
            let idx = usize::try_from(index).unwrap_or_default();
            let set_idx = usize::try_from(set_index).unwrap_or_default();
            let default_rest_seconds = state.settings.default_rest_seconds;

            if let Some(draft) = state.template_draft.exercises.get_mut(idx) {
                if let Some(set) = draft.sets.get_mut(set_idx) {
                    if let Ok(reps) = parse_optional_i32(&reps) {
                        set.reps = reps;
                    }
                    if let Ok(weight) = parse_optional_f32(&weight) {
                        set.weight = weight;
                    }
                    if let Ok(rest) = parse_i32(&rest, default_rest_seconds) {
                        set.rest_seconds = rest;
                    }
                }
            }
            Ok(())
        })();
        // Silent update only — no UI refresh to prevent TextInput losing focus.
        if let Err(error) = outcome {
            println!("Error updating set: {}", error);
        }
    });

    let _weak = ui.as_weak();
    let state_for_update_set_range = state.clone();
    ui.on_update_set_range(move |index, start_set_index, end_set_index, field, value| {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_update_set_range.borrow_mut();
            let idx = usize::try_from(index).unwrap_or_default();
            let start_idx = usize::try_from(start_set_index.min(end_set_index)).unwrap_or_default();
            let end_idx = usize::try_from(start_set_index.max(end_set_index)).unwrap_or_default();

            if let Some(draft) = state.template_draft.exercises.get_mut(idx) {
                let clamped_end = end_idx.min(draft.sets.len().saturating_sub(1));
                for set in draft
                    .sets
                    .iter_mut()
                    .skip(start_idx)
                    .take(clamped_end.saturating_sub(start_idx) + 1)
                {
                    match field.as_str() {
                        "reps" => {
                            if let Ok(reps) = parse_optional_i32(&value) {
                                set.reps = reps;
                            }
                        }
                        "weight" => {
                            if let Ok(weight) = parse_optional_f32(&value) {
                                set.weight = weight;
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = outcome {
            println!("Error updating set range: {}", error);
        }
    });

    let weak = ui.as_weak();
    let state_for_clear_draft = state.clone();
    ui.on_clear_draft(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_clear_draft.borrow_mut();
            state.template_draft = TemplateDraft::default();
            state.status_message = "".into();
            Ok(())
        })();
        if let Some(ui_instance) = weak.upgrade() {
            ui_instance.set_template_name("".into());
            ui_instance.set_template_icon("chest".into());
            ui_instance.set_template_active_days(slint::ModelRc::new(slint::VecModel::from(
                vec![false; 7],
            )));
        }
        with_partial_refresh(&weak, &state_for_clear_draft, outcome, |ui, state| {
            refresh_planner(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_save_template = state.clone();
    ui.on_save_template(move |name, icon, assigned_days| {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_save_template.borrow_mut();
            let trimmed_name = name.trim();
            state.template_draft.name = if trimmed_name.is_empty() {
                "Unnamed Template".to_string()
            } else {
                trimmed_name.to_string()
            };
            state.template_draft.icon = icon.to_string();
            state.template_draft.assigned_days = normalize_days(assigned_days.as_str());
            let saved_name = state.template_draft.name.clone();
            state.db.save_template(&state.template_draft)?;
            state.template_draft = TemplateDraft::default();
            state.status_message = format!("Saved template {}.", saved_name);
            Ok(())
        })();
        if let Some(ui_instance) = weak.upgrade() {
            ui_instance.set_template_name("".into());
            ui_instance.set_template_icon("chest".into());
            ui_instance.set_template_active_days(slint::ModelRc::new(slint::VecModel::from(
                vec![false; 7],
            )));
        }
        with_partial_refresh(&weak, &state_for_save_template, outcome, |ui, state| {
            let _ = refresh_home(ui, state);
            refresh_planner(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_delete_template = state.clone();
    ui.on_delete_template(move |template_id| {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_delete_template.borrow_mut();
            state.db.delete_template(i64::from(template_id))?;
            state.status_message = "Template deleted.".into();
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_delete_template, outcome, |ui, state| {
            let _ = refresh_home(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_edit_template = state.clone();
    ui.on_edit_template(move |template_id| {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_edit_template.borrow_mut();
            let template = state
                .db
                .load_template(i64::from(template_id))?
                .context("Template not found")?;

            let exercises = template
                .exercises
                .into_iter()
                .map(|ex| crate::models::TemplateDraftExercise {
                    exercise_id: ex.exercise_id,
                    exercise_name: ex.exercise_name,
                    set_type: ex
                        .planned_sets
                        .first()
                        .map(|s| s.set_type.clone())
                        .unwrap_or(SetType::Normal),
                    weight_type: ex
                        .planned_sets
                        .first()
                        .map(|s| s.weight_type.clone())
                        .unwrap_or(WeightType::Kg),
                    sets: ex
                        .planned_sets
                        .into_iter()
                        .map(|set| crate::models::DraftSet {
                            reps: set.reps,
                            duration_seconds: set.duration_seconds,
                            weight: set.weight,
                            rest_seconds: set.rest_seconds,
                        })
                        .collect(),
                })
                .collect();

            state.template_draft = TemplateDraft {
                id: Some(template.id),
                name: template.name.clone(),
                icon: template.icon.clone(),
                assigned_days: template.assigned_days.clone(),
                exercises,
            };

            Ok(())
        })();
        if let Some(ui_instance) = weak.upgrade() {
            let state = state_for_edit_template.borrow();
            ui_instance.set_template_name(state.template_draft.name.clone().into());
            ui_instance.set_template_icon(state.template_draft.icon.clone().into());

            let mut days = vec![false; 7];
            for day in &state.template_draft.assigned_days {
                match day.as_str() {
                    "Mon" | "Monday" => days[0] = true,
                    "Tue" | "Tuesday" => days[1] = true,
                    "Wed" | "Wednesday" => days[2] = true,
                    "Thu" | "Thursday" => days[3] = true,
                    "Fri" | "Friday" => days[4] = true,
                    "Sat" | "Saturday" => days[5] = true,
                    "Sun" | "Sunday" => days[6] = true,
                    _ => {}
                }
            }
            ui_instance.set_template_active_days(slint::ModelRc::new(slint::VecModel::from(days)));
        }
        with_partial_refresh(&weak, &state_for_edit_template, outcome, |ui, state| {
            refresh_planner(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_start = state.clone();
    ui.on_start_workout(move |template_id| {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_start.borrow_mut();
            let template = state
                .db
                .load_template(i64::from(template_id))?
                .context("template not found")?;

            // Bulk-load all reference labels for exercises in this template.
            let exercise_ids: Vec<i64> = template.exercises.iter().map(|e| e.exercise_id).collect();
            let reference_map = state.db.bulk_reference_labels(&exercise_ids)?;

            let exercises = template
                .exercises
                .iter()
                .map(|exercise| {
                    let description = state.db.exercise_description(exercise.exercise_id)?;
                    let sets = exercise
                        .planned_sets
                        .iter()
                        .map(|set| {
                            let reference_label = reference_map
                                .get(&(exercise.exercise_id, set.set_number))
                                .cloned()
                                .unwrap_or_default();
                            Ok(ActiveSet {
                                set_number: set.set_number,
                                set_type: set.set_type.clone(),
                                planned_reps: set.reps,
                                planned_weight: set.weight,
                                planned_duration: set.duration_seconds,
                                weight_type: set.weight_type.clone(),
                                rest_seconds: set.rest_seconds,
                                actual_reps: None,
                                actual_weight: None,
                                actual_duration: None,
                                completed: false,
                                is_pr: false,
                                reference_label,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;

                    Ok(ActiveExercise {
                        exercise_id: exercise.exercise_id,
                        name: exercise.exercise_name.clone(),
                        description,
                        sets,
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            state.active_workout = Some(ActiveWorkout {
                template_id: template.id,
                template_name: template.name.clone(),
                icon: template.icon.clone(),
                started_at: Local::now(),
                exercises,
            });
            state.selected_exercise_index = 0;
            state.status_message = format!("Started workout {}.", template.name);
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_start, outcome, |ui, state| {
            let _ = refresh_home(ui, state);
            refresh_workout(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_select = state.clone();
    ui.on_select_active_exercise(move |index| {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_select.borrow_mut();
            let idx = usize::try_from(index).unwrap_or_default();
            if let Some(workout) = state.active_workout.as_ref() {
                if idx < workout.exercises.len() {
                    state.selected_exercise_index = idx;
                }
            }
            Ok(())
        })();
        // Only the workout screen needs updating.
        with_partial_refresh(&weak, &state_for_select, outcome, |ui, state| {
            refresh_workout(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_complete = state.clone();
    ui.on_complete_set(move |exercise_index, set_index, reps, weight, duration| {
        let mut triggered_rest = 0;
        let outcome = (|| -> Result<()> {
            let mut state = state_for_complete.borrow_mut();
            let exercise_idx = usize::try_from(exercise_index).unwrap_or_default();
            let set_idx = usize::try_from(set_index).unwrap_or_default();
            let db = state.db.clone();
            let mut next_selected_index = exercise_idx;
            let status_message = {
                let workout = state.active_workout.as_mut().context("no active workout")?;
                let exercise = workout
                    .exercises
                    .get_mut(exercise_idx)
                    .context("exercise index out of bounds")?;

                let exercise_name = exercise.name.clone();
                let exercise_id = exercise.exercise_id;

                let (set_number, rest_seconds, is_pr) = {
                    let set = exercise
                        .sets
                        .get_mut(set_idx)
                        .context("set index out of bounds")?;

                    if set.completed {
                        return Ok(());
                    }

                    set.actual_reps = parse_optional_i32(&reps)?.or(set.planned_reps);
                    set.actual_weight = parse_optional_f32(&weight)?.or(set.planned_weight);
                    set.actual_duration = parse_optional_i32(&duration)?.or(set.planned_duration);
                    let prs = db.evaluate_prs(
                        exercise_id,
                        set.actual_reps,
                        set.actual_weight,
                        set.actual_duration,
                    )?;
                    set.is_pr = !prs.is_empty();
                    set.completed = true;
                    (set.set_number, set.rest_seconds, set.is_pr)
                };

                let exercise_done = exercise.sets.iter().all(|planned| planned.completed);
                if exercise_done {
                    if let Some(next_idx) = workout
                        .exercises
                        .iter()
                        .position(|entry| entry.sets.iter().any(|candidate| !candidate.completed))
                    {
                        next_selected_index = next_idx;
                    }
                }

                triggered_rest = rest_seconds;

                if is_pr {
                    format!(
                        "{} set {} logged. New PR with {} rest next.",
                        exercise_name,
                        set_number,
                        format_duration(rest_seconds)
                    )
                } else {
                    format!(
                        "{} set {} logged. Rest {}.",
                        exercise_name,
                        set_number,
                        format_duration(rest_seconds)
                    )
                }
            };

            state.selected_exercise_index = next_selected_index;
            state.status_message = status_message;
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_complete, outcome, |ui, state| {
            refresh_workout(ui, state);
            refresh_status(ui, state);
        });
        if let Some(ui) = weak.upgrade() {
            if triggered_rest > 0 {
                ui.set_active_rest_duration(triggered_rest);
                ui.set_active_rest_timer(triggered_rest);
            }
        }
    });

    let weak = ui.as_weak();
    let state_for_add_active_set = state.clone();
    ui.on_add_active_set(move |exercise_index| {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_add_active_set.borrow_mut();
            let ex_idx = usize::try_from(exercise_index).unwrap_or_default();
            let workout = state.active_workout.as_mut().context("no active workout")?;
            let exercise = workout
                .exercises
                .get_mut(ex_idx)
                .context("exercise index out of bounds")?;
            let last_set = exercise
                .sets
                .last()
                .cloned()
                .context("exercise has no sets")?;
            let new_set_number = last_set.set_number + 1;
            exercise.sets.push(ActiveSet {
                set_number: new_set_number,
                set_type: last_set.set_type.clone(),
                planned_reps: last_set.planned_reps,
                planned_weight: last_set.planned_weight,
                planned_duration: last_set.planned_duration,
                weight_type: last_set.weight_type.clone(),
                rest_seconds: last_set.rest_seconds,
                actual_reps: None,
                actual_weight: None,
                actual_duration: None,
                completed: false,
                is_pr: false,
                reference_label: last_set.reference_label.clone(),
            });
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_add_active_set, outcome, |ui, state| {
            refresh_workout(ui, state);
        });
    });

    let weak = ui.as_weak();
    let state_for_period = state.clone();
    let state_for_finish = state;
    ui.on_finish_workout(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_finish.borrow_mut();
            let workout = state.active_workout.take().context("no active workout")?;
            let summary = state.db.save_completed_workout(&workout)?;
            state.selected_exercise_index = 0;
            state.status_message = format!(
                "Finished {} in {} with {:.0} total volume and {} PRs.",
                summary.template_name,
                format_duration(summary.duration_seconds),
                summary.total_volume,
                summary.pr_count
            );
            Ok(())
        })();
        with_partial_refresh(&weak, &state_for_finish, outcome, |ui, state| {
            let _ = refresh_home(ui, state);
            refresh_workout(ui, state);
            refresh_stats(ui, state);
            refresh_status(ui, state);
        });
    });

    let weak = ui.as_weak();
    ui.on_stats_period_changed(move || {
        if let Some(ui) = weak.upgrade() {
            refresh_stats(&ui, &state_for_period.borrow());
        }
    });
}

/// Apply error to state, then call the provided partial-refresh closure.
fn with_partial_refresh(
    weak: &slint::Weak<MainWindow>,
    state: &Rc<RefCell<AppState>>,
    outcome: Result<()>,
    refresh_fn: impl Fn(&MainWindow, &AppState),
) {
    {
        let mut state = state.borrow_mut();
        if let Err(error) = outcome {
            state.status_message = format!("Error: {error}");
        }
    }
    if let Some(ui) = weak.upgrade() {
        refresh_fn(&ui, &state.borrow());
    }
}

/// Ensures that all parts of the user interface are synchronized with the most recent data from the database and internal state.
fn refresh_ui(ui: &MainWindow, state: &mut AppState) -> Result<()> {
    sync_settings_ui(ui, state);
    refresh_exercises(ui, state);
    refresh_home(ui, state)?;
    refresh_planner(ui, state);
    refresh_workout(ui, state);
    refresh_stats(ui, state);
    refresh_status(ui, state);
    Ok(())
}

// ---------------------------------------------------------------------------
// Partial refresh functions — each touches only one slice of the UI.
// ---------------------------------------------------------------------------

/// Loads embedded inline SVGs for rendering icons (lazy load once).
fn get_svg_strings() -> &'static std::collections::HashMap<i64, (String, String)> {
    static SVGS: std::sync::OnceLock<std::collections::HashMap<i64, (String, String)>> =
        std::sync::OnceLock::new();
    SVGS.get_or_init(|| {
        let json = include_str!("../exercises/optimized_exercises.json");

        #[derive(serde::Deserialize)]
        struct Ex {
            id: i64,
            svg_images: Option<Vec<String>>,
        }

        let exercises: Vec<Ex> = serde_json::from_str(json).unwrap_or_default();
        let mut map = std::collections::HashMap::new();
        for ex in exercises {
            if let Some(mut svgs) = ex.svg_images {
                if svgs.len() >= 2 {
                    let t = svgs.pop().unwrap();
                    let r = svgs.pop().unwrap();
                    map.insert(ex.id, (r, t));
                }
            }
        }
        map
    })
}

/// Decodes matching SVG strings to Slint Images. Results are permanently cached in thread-local storage.
fn get_cached_images(id: i64) -> Option<(slint::Image, slint::Image)> {
    std::thread_local! {
        static IMAGE_CACHE: std::cell::RefCell<std::collections::HashMap<i64, (slint::Image, slint::Image)>> = std::cell::RefCell::new(std::collections::HashMap::new());
    }

    IMAGE_CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        if let Some(imgs) = map.get(&id) {
            return Some(imgs.clone());
        }

        if let Some((r_str, t_str)) = get_svg_strings().get(&id) {
            let r = slint::Image::load_from_svg_data(r_str.as_bytes()).unwrap_or_default();
            let t = slint::Image::load_from_svg_data(t_str.as_bytes()).unwrap_or_default();
            map.insert(id, (r.clone(), t.clone()));
            Some((r, t))
        } else {
            None
        }
    })
}

/// Rebuild the exercise list from the database.
fn refresh_exercises(ui: &MainWindow, state: &AppState) {
    let search_query = ui.get_search_query().to_string().to_lowercase();
    let filter_muscle = ui.get_filter_muscle().to_string();
    let sort_by = ui.get_sort_by().to_string();

    let mut exercise_rows: Vec<ExerciseRow> = state
        .db
        .list_exercises()
        .unwrap_or_default()
        .into_iter()
        .filter(|ex| {
            if !search_query.is_empty() && !ex.name.to_lowercase().contains(&search_query) {
                return false;
            }
            if filter_muscle != "All Muscles" && ex.muscle_group.label() != filter_muscle {
                return false;
            }
            true
        })
        .map(|exercise| {
            let (has_images, image_relaxed, image_tension) = if exercise.source == "system" {
                if let Some((r, t)) = get_cached_images(exercise.id) {
                    (true, r, t)
                } else {
                    (false, slint::Image::default(), slint::Image::default())
                }
            } else {
                (false, slint::Image::default(), slint::Image::default())
            };

            ExerciseRow {
                id: to_i32(exercise.id),
                name: exercise.name.clone().into(),
                meta: format!(
                    "{} • {}",
                    exercise.muscle_group.label(),
                    exercise.equipment.label(),
                )
                .into(),
                description: exercise.description.clone().into(),
                source: exercise.source.clone().into(),
                has_images,
                image_relaxed,
                image_tension,
            }
        })
        .collect();

    if sort_by == "A-Z" {
        exercise_rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    } else {
        exercise_rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    }

    ui.set_exercises(ModelRc::new(VecModel::from(exercise_rows)));
}

/// Rebuild the Home tab: templates, sessions, schedule, hero card, week progress.
fn refresh_home(ui: &MainWindow, state: &AppState) -> Result<()> {
    let templates = state.db.list_templates()?;
    let recent_sessions = state.db.recent_sessions(5)?;
    let schedule = state.db.schedule()?;

    ui.set_home_has_plans(!templates.is_empty());

    let template_rows = templates
        .iter()
        .map(|template| TemplateCard {
            id: to_i32(template.id),
            name: template.name.clone().into(),
            icon: template.icon.clone().into(),
            days: if template.assigned_days.is_empty() {
                SharedString::from("Free training")
            } else {
                SharedString::from(template.assigned_days.join(", "))
            },
            info: format!("{} exercises", template.exercises.len()).into(),
            created_at: template.created_at.clone().into(),
        })
        .collect::<Vec<_>>();
    let template_rows = if template_rows.is_empty() {
        vec![TemplateCard {
            id: -1,
            name: "Build your first plan".into(),
            icon: "chest".into(),
            days: "".into(),
            info: "".into(),
            created_at: "".into(),
        }]
    } else {
        template_rows
    };
    ui.set_templates(ModelRc::new(VecModel::from(template_rows)));

    let session_rows = recent_sessions
        .iter()
        .map(|session| SessionCard {
            icon: session.icon.clone().into(),
            title: session.template_name.clone().into(),
            subtitle: format_relative_day(&session.finished_at)
                .to_uppercase()
                .into(),
            metric: format!(
                "{} - {:.0} KG - {} PRS",
                format_duration(session.duration_seconds).to_uppercase(),
                session.total_volume,
                session.pr_count
            )
            .into(),
        })
        .collect::<Vec<_>>();
    let session_rows = if session_rows.is_empty() {
        vec![SessionCard {
            icon: "".into(),
            title: "START YOUR FIRST SESSION".into(),
            subtitle: "READY WHEN YOU ARE".into(),
            metric: "TRACK WORKOUTS TO BUILD HISTORY".into(),
        }]
    } else {
        session_rows
    };
    ui.set_recent_sessions(ModelRc::new(VecModel::from(session_rows)));

    let schedule_rows = schedule
        .iter()
        .map(|entry| ScheduleCard {
            day: entry.day.clone().into(),
            title: entry.workout_name.clone().into(),
            detail: entry.detail.clone().into(),
        })
        .collect::<Vec<_>>();
    let schedule_rows = if schedule_rows.is_empty() {
        vec![ScheduleCard {
            day: "PLAN".into(),
            title: "Assign training days in Builder".into(),
            detail: "Example: Mon push, Wed pull, Fri legs.".into(),
        }]
    } else {
        schedule_rows
    };
    ui.set_schedule(ModelRc::new(VecModel::from(schedule_rows)));

    ui.set_next_workout_label(next_workout_label(&schedule));
    ui.set_streak_label(streak_label(&recent_sessions));
    ui.set_header_greeting("HELLO, ATHLETE!".into());
    ui.set_header_motto("PUSH YOUR LIMITS TODAY".into());
    ui.set_header_date(
        Local::now()
            .format("%b %d")
            .to_string()
            .to_uppercase()
            .into(),
    );
    ui.set_week_progress(ModelRc::new(VecModel::from(week_progress_rows(
        &schedule,
        &recent_sessions,
    ))));

    let current_day = chrono::Local::now().format("%A").to_string();
    let today_template = templates.iter().find(|t| {
        t.assigned_days
            .iter()
            .any(|d| d.eq_ignore_ascii_case(&current_day))
    });

    if let Some(template) = today_template {
        let cycle = if template.assigned_days.is_empty() {
            "CUSTOM SETUP".to_string()
        } else {
            template
                .assigned_days
                .iter()
                .map(|day| day.chars().take(3).collect::<String>().to_uppercase())
                .collect::<Vec<_>>()
                .join(" / ")
        };
        ui.set_featured_template_id(to_i32(template.id));
        ui.set_featured_template_name(template.name.to_uppercase().into());
        ui.set_featured_template_icon(template.icon.clone().into());
        ui.set_featured_template_meta(
            format!("{} EXERCISES - {}", template.exercises.len(), cycle).into(),
        );
        ui.set_featured_template_detail("".into());
        ui.set_featured_template_cta("START SESSION".into());
    } else {
        ui.set_featured_template_id(-1);
        ui.set_featured_template_name("REST DAY".into());
        ui.set_featured_template_icon("chest".into());
        ui.set_featured_template_meta("".into());
        ui.set_featured_template_detail(
            "Nothing here today. Keep your day off or add a plan for today.".into(),
        );
        ui.set_featured_template_cta("OPEN PLANNER".into());
    }

    Ok(())
}

/// Rebuild the template draft panel (Planner tab) — no DB access.
fn refresh_planner(ui: &MainWindow, state: &AppState) {
    let draft_rows = state
        .template_draft
        .exercises
        .iter()
        .enumerate()
        .map(|(index, draft)| {
            let mut sets_model = Vec::new();
            for s in &draft.sets {
                sets_model.push(DraftSetRow {
                    reps: s.reps.map(|v| v.to_string()).unwrap_or_default().into(),
                    weight: s.weight.map(|v| v.to_string()).unwrap_or_default().into(),
                    rest: s.rest_seconds.to_string().into(),
                });
            }

            let first_set = draft.sets.first();

            DraftExerciseRow {
                index: index as i32,
                title: draft.exercise_name.clone().into(),
                sets_count: draft.sets.len() as i32,
                sets: slint::ModelRc::new(slint::VecModel::from(sets_model)),
                detail: format!(
                    "{} x {} • {} • rest {}",
                    draft.sets.len(),
                    match draft.set_type {
                        SetType::Timed => first_set
                            .and_then(|s| s.duration_seconds)
                            .map(|value| format!("{}s", value))
                            .unwrap_or_else(|| "timed".into()),
                        _ => first_set
                            .and_then(|s| s.reps)
                            .map(|value| format!("{} reps", value))
                            .unwrap_or_else(|| "auto reps".into()),
                    },
                    match first_set.and_then(|s| s.weight) {
                        Some(value) => format!("{value:.1} {}", draft.weight_type.label()),
                        None => draft.weight_type.label().into(),
                    },
                    format_duration(first_set.map(|s| s.rest_seconds).unwrap_or(0))
                )
                .into(),
            }
        })
        .collect::<Vec<_>>();
    ui.set_draft_exercises(ModelRc::new(VecModel::from(draft_rows)));
}

/// Rebuild the Workout screen — no DB access, reads only in-memory active_workout.
fn refresh_workout(ui: &MainWindow, state: &AppState) {
    if let Some(workout) = state.active_workout.as_ref() {
        ui.set_has_active_workout(true);
        ui.set_active_workout_title(workout.template_name.clone().into());
        ui.set_active_workout_subtitle(
            format!(
                "Started {} • {} exercises",
                workout.started_at.format("%H:%M"),
                workout.exercises.len()
            )
            .into(),
        );

        let selected_index = state
            .selected_exercise_index
            .min(workout.exercises.len().saturating_sub(1));
        ui.set_current_exercise_index(selected_index as i32);
        let current = workout.exercises.get(selected_index);
        let previous = selected_index
            .checked_sub(1)
            .and_then(|index| workout.exercises.get(index));
        let next = workout.exercises.get(selected_index + 1);

        ui.set_previous_exercise_title(
            previous
                .map(|entry| entry.name.clone())
                .unwrap_or_else(|| "-".into())
                .into(),
        );
        ui.set_next_exercise_title(
            next.map(|entry| entry.name.clone())
                .unwrap_or_else(|| "-".into())
                .into(),
        );
        ui.set_current_exercise_title(
            current
                .map(|entry| entry.name.clone())
                .unwrap_or_else(|| "No exercise selected".into())
                .into(),
        );
        ui.set_current_exercise_description(
            current
                .map(|entry| entry.description.clone())
                .unwrap_or_else(|| "".into())
                .into(),
        );
        ui.set_workout_progress(workout_progress_label(workout).into());
        ui.set_rest_hint(current_rest_hint(current));

        let exercise_rows = workout
            .exercises
            .iter()
            .enumerate()
            .map(|(index, exercise)| WorkoutExerciseRow {
                index: index as i32,
                title: exercise.name.clone().into(),
                subtitle: format!(
                    "{} / {} sets complete",
                    exercise.sets.iter().filter(|set| set.completed).count(),
                    exercise.sets.len()
                )
                .into(),
                badge: if index == selected_index {
                    SharedString::from("Current")
                } else if exercise.sets.iter().all(|set| set.completed) {
                    SharedString::from("Done")
                } else {
                    SharedString::from("Queued")
                },
            })
            .collect::<Vec<_>>();
        ui.set_workout_exercises(ModelRc::new(VecModel::from(exercise_rows)));

        let set_rows = current
            .map(|exercise| {
                let last_done_idx = exercise.sets.iter().rposition(|s| s.completed);
                let first_open_idx = exercise.sets.iter().position(|s| !s.completed);
                let n = exercise.sets.len();
                exercise
                    .sets
                    .iter()
                    .enumerate()
                    .map(|(set_index, set)| {
                        let status = if set.completed && set.is_pr {
                            SharedString::from("PR")
                        } else if set.completed {
                            SharedString::from("Done")
                        } else if Some(set_index) == first_open_idx {
                            SharedString::from("Active")
                        } else {
                            SharedString::from("Open")
                        };

                        let rest_after = Some(set_index) == last_done_idx;

                        let has_next_exercise = next.is_some();
                        let rest_label = if !set.completed
                            && (set_index < n.saturating_sub(1) || has_next_exercise)
                        {
                            format_rest_mss(set.rest_seconds).into()
                        } else {
                            SharedString::from("")
                        };

                        let display_kg = if set.completed {
                            set.actual_weight
                                .map(|v| format!("{:.1}", v))
                                .unwrap_or_default()
                                .into()
                        } else {
                            set.planned_weight
                                .map(|v| format!("{:.1}", v))
                                .unwrap_or_default()
                                .into()
                        };

                        let display_reps = if set.completed {
                            set.actual_reps
                                .map(|v| v.to_string())
                                .unwrap_or_default()
                                .into()
                        } else {
                            set.planned_reps
                                .map(|v| v.to_string())
                                .unwrap_or_default()
                                .into()
                        };

                        WorkoutSetRow {
                            exercise_index: selected_index as i32,
                            set_index: set_index as i32,
                            title: format!("{}", set.set_number).into(),
                            plan: format_set_plan(set).into(),
                            actual: format_set_actual(set).into(),
                            reference: set.reference_label.clone().into(),
                            status,
                            rest_after,
                            rest_label,
                            display_kg,
                            display_reps,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        ui.set_workout_sets(ModelRc::new(VecModel::from(set_rows)));
    } else {
        ui.set_has_active_workout(false);
        ui.set_active_workout_title("No active workout".into());
        ui.set_active_workout_subtitle(
            "Pick a template from Home to start a guided session.".into(),
        );
        ui.set_previous_exercise_title("-".into());
        ui.set_next_exercise_title("-".into());
        ui.set_current_exercise_title("No exercise selected".into());
        ui.set_current_exercise_description("When a workout starts, the current exercise and its last logged reference appear here.".into());
        ui.set_workout_progress("0 / 0 sets complete".into());
        ui.set_rest_hint("Rest timer adopts the template rest after every completed set.".into());
        ui.set_workout_exercises(ModelRc::new(VecModel::from(
            Vec::<WorkoutExerciseRow>::new(),
        )));
        ui.set_workout_sets(ModelRc::new(VecModel::from(Vec::<WorkoutSetRow>::new())));
    }
}

/// Push the current status message to the UI (used for flash feedback alerts).
fn refresh_status(ui: &MainWindow, state: &AppState) {
    ui.set_status_message(state.status_message.clone().into());
}

/// Formats duration (in seconds) precisely into an `m:ss` string format for the UI.
fn format_rest_mss(seconds: i32) -> String {
    let m = seconds / 60;
    let s = seconds % 60;
    format!("{}:{:02}", m, s)
}

/// Parses a string into an integer safely; returns `fallback` if the text is empty.
fn parse_i32(text: &str, fallback: i32) -> Result<i32> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(fallback);
    }
    Ok(trimmed.parse::<i32>()?)
}

/// Parses optional integer text inputs (e.g. reps), converting empty strings to `None`.
fn parse_optional_i32(text: &str) -> Result<Option<i32>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(trimmed.parse::<i32>()?))
}

/// Parses an optional float for inputs like weight. Replaces commas with dots to support regional keyboard variations.
fn parse_optional_f32(text: &str) -> Result<Option<f32>> {
    let trimmed = text.trim().replace(',', ".");
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(trimmed.parse::<f32>()?))
}

/// Matches the incoming UI string to the strong backend `WeightType` enum.
fn parse_weight_type(value: &str) -> WeightType {
    match value.trim() {
        "Lbs" => WeightType::Lbs,
        "Bodyweight" => WeightType::Bodyweight,
        "Assisted" => WeightType::Assisted,
        "BW+" => WeightType::BwPlus,
        _ => WeightType::Kg,
    }
}

/// Safely converts i64 database ID sizes back down to interoperable i32 sizes for UI display logic.
fn to_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

/// Reads the next scheduled training day and workout explicitly from the configured schedule.
fn next_workout_label(schedule: &[models::ScheduleEntry]) -> SharedString {
    schedule
        .first()
        .map(|entry| format!("{} - {}", entry.day, entry.workout_name).into())
        .unwrap_or_else(|| SharedString::from("No workout scheduled yet"))
}

/// Iterates through history logs backwards from today to construct the user's current workout streak.
fn streak_label(recent_sessions: &[models::WorkoutSessionSummary]) -> SharedString {
    let mut unique_days = recent_sessions
        .iter()
        .filter_map(|session| {
            NaiveDate::parse_from_str(&session.finished_at[..10], "%Y-%m-%d").ok()
        })
        .collect::<Vec<_>>();
    unique_days.sort_unstable();
    unique_days.dedup();
    unique_days.reverse();

    let mut streak = 0;
    let mut current_day = Local::now().date_naive();
    for day in unique_days {
        if day == current_day || day == current_day.pred_opt().unwrap_or(current_day) {
            streak += 1;
            current_day = day.pred_opt().unwrap_or(day);
        } else {
            break;
        }
    }
    format!("Streak: {} days", streak).into()
}

/// Creates a visually readable string demonstrating how many sets are completed vs total queued up.
fn workout_progress_label(workout: &ActiveWorkout) -> String {
    let done = workout
        .exercises
        .iter()
        .flat_map(|exercise| exercise.sets.iter())
        .filter(|set| set.completed)
        .count();
    let total = workout
        .exercises
        .iter()
        .flat_map(|exercise| exercise.sets.iter())
        .count();
    format!("{} / {} sets complete", done, total)
}

/// Builds a hint indicating what the rest timer will be set to when completing the next active set.
fn current_rest_hint(current: Option<&ActiveExercise>) -> SharedString {
    current
        .and_then(|exercise| {
            exercise.sets.iter().find(|set| !set.completed).map(|set| {
                format!(
                    "Next rest cue: {} after set {}.",
                    format_duration(set.rest_seconds),
                    set.set_number
                )
            })
        })
        .unwrap_or_else(|| "All sets logged for the selected exercise.".into())
        .into()
}

/// Generates seven items representing M-Sun to indicate on which days the user trained during this week.
fn week_progress_rows(
    _schedule: &[models::ScheduleEntry],
    recent_sessions: &[models::WorkoutSessionSummary],
) -> Vec<WeekdayProgress> {
    let labels = [
        ("Monday", "MON"),
        ("Tuesday", "TUE"),
        ("Wednesday", "WED"),
        ("Thursday", "THU"),
        ("Friday", "FRI"),
        ("Saturday", "SAT"),
        ("Sunday", "SUN"),
    ];
    let today = Local::now().format("%A").to_string();

    labels
        .into_iter()
        .map(|(full, short)| {
            let session_for_day = recent_sessions.iter().find(|session| {
                chrono::NaiveDateTime::parse_from_str(&session.finished_at, "%Y-%m-%d %H:%M")
                    .ok()
                    .map(|stamp| stamp.format("%A").to_string().eq_ignore_ascii_case(full))
                    .unwrap_or(false)
            });
            let trained = session_for_day.is_some();
            let icon = session_for_day
                .map(|s| s.icon.clone())
                .unwrap_or_else(|| "chest".to_string());

            WeekdayProgress {
                label: short.into(),
                icon: icon.into(),
                active: trained,
                current: today.eq_ignore_ascii_case(full),
            }
        })
        .collect()
}

// ── Stats refresh ─────────────────────────────────────────────────────────

fn refresh_stats(ui: &MainWindow, state: &AppState) {
    let period_str = ui.get_stats_period().to_string();
    let period_days: i64 = match period_str.as_str() {
        "LAST 7 DAYS" => 7,
        "LAST 30 DAYS" => 30,
        "LAST 365 DAYS" => 365,
        _ => 9999, // ALL TIME
    };

    // History
    let sessions = state
        .db
        .stats_sessions_in_period(period_days)
        .unwrap_or_default();
    let history_rows: Vec<SessionCard> = sessions
        .iter()
        .take(10)
        .map(|s| SessionCard {
            icon: s.icon.clone().into(),
            title: format!(
                "{} {}",
                s.template_name,
                format_duration(s.duration_seconds)
            )
            .into(),
            subtitle: format_relative_day(&s.finished_at).to_uppercase().into(),
            metric: format!("{:.0} KG VOL", s.total_volume).into(),
        })
        .collect();
    ui.set_stats_history(ModelRc::new(VecModel::from(history_rows)));

    // Workouts this month
    ui.set_stats_workouts_month(sessions.len() as i32);

    // New PRs
    let pr_count = state.db.stats_pr_count_in_period(period_days).unwrap_or(0);
    ui.set_stats_new_prs(pr_count as i32);

    // Calendar
    let now = Local::now();
    let year = now.year();
    let month = now.month();
    let month_name = now.format("%B %Y").to_string().to_uppercase();
    ui.set_stats_calendar_month_label(month_name.into());

    // First weekday of the month (0 = Sunday for our S M T W T F S layout)
    if let Some(first_day) = NaiveDate::from_ymd_opt(year, month, 1) {
        // chrono: Monday=0 .. Sunday=6 — our grid: Sunday=0 .. Saturday=6
        let weekday_monday_based = first_day.weekday().num_days_from_monday();
        let weekday_sunday_based = (weekday_monday_based + 1) % 7;
        ui.set_stats_calendar_first_weekday(weekday_sunday_based as i32);
    }

    // Days in month: step to next month, subtract 1
    let days_in_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .and_then(|d| d.pred_opt())
    .map(|d| d.day())
    .unwrap_or(30);
    ui.set_stats_calendar_days_in_month(days_in_month as i32);

    let active_days = state
        .db
        .stats_calendar_days(year, month)
        .unwrap_or_default();
    let today_day = now.day();
    let calendar_rows: Vec<StatsCalendarDay> = (1..=days_in_month)
        .map(|d| StatsCalendarDay {
            day: d as i32,
            active: active_days.contains(&d),
            is_today: d == today_day,
        })
        .collect();
    ui.set_stats_calendar_days(ModelRc::new(VecModel::from(calendar_rows)));

    // Top exercises
    let top_ex = state.db.stats_top_exercises(5).unwrap_or_default();
    let exercise_rows: Vec<StatsExerciseRow> = top_ex
        .into_iter()
        .map(|(name, count)| StatsExerciseRow {
            name: name.to_uppercase().into(),
            count_label: format!("{}x", count).into(),
        })
        .collect();
    ui.set_stats_top_exercises(ModelRc::new(VecModel::from(exercise_rows)));

    // PR Progression
    let pr_prog = state.db.stats_pr_progression(5).unwrap_or_default();
    let pr_rows: Vec<StatsPrRow> = pr_prog
        .into_iter()
        .map(|(name, current, prev)| {
            let diff = current - prev;
            let trend = if prev == 0.0 {
                "NEW".to_string()
            } else if diff > 0.0 {
                format!("+{:.0}%", (diff / prev) * 100.0)
            } else {
                "STABLE".to_string()
            };
            StatsPrRow {
                name: name.to_uppercase().into(),
                current_weight: format!("{:.0} kg", current).into(),
                last_weight: format!("{:.0} kg", prev).into(),
                trend: trend.into(),
            }
        })
        .collect();
    ui.set_stats_pr_progression(ModelRc::new(VecModel::from(pr_rows)));

    // Plan session counts
    let plan_counts = state.db.stats_plan_session_counts().unwrap_or_default();
    let plan_rows: Vec<StatsPlanRow> = plan_counts
        .into_iter()
        .map(|(name, count)| StatsPlanRow {
            name: name.to_uppercase().into(),
            session_count: format!("{} SESSIONS", count).into(),
        })
        .collect();
    ui.set_stats_plan_rows(ModelRc::new(VecModel::from(plan_rows)));
}
