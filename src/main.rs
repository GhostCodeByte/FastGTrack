mod db;
mod models;

use std::{cell::RefCell, rc::Rc};

use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};
use db::Database;
use models::{
    ActiveExercise, ActiveSet, ActiveWorkout, Equipment, ExerciseInput, MuscleGroup, SetType,
    TemplateDraft, TemplateDraftExercise, WeightType, format_duration, format_relative_day,
    format_set_actual, format_set_plan, normalize_days,
};
use slint::{ModelRc, SharedString, VecModel};

slint::include_modules!();

struct AppState {
    db: Rc<Database>,
    template_draft: TemplateDraft,
    active_workout: Option<ActiveWorkout>,
    selected_exercise_index: usize,
    status_message: String,
}

impl AppState {
    fn new(db: Rc<Database>) -> Self {
        Self {
            db,
            template_draft: TemplateDraft::default(),
            active_workout: None,
            selected_exercise_index: 0,
            status_message: "Workspace ready.".into(),
        }
    }
}

fn main() -> Result<()> {
    let db = Rc::new(Database::open("fastgtrack.db").context("failed to bootstrap FastGTrack DB")?);
    let state = Rc::new(RefCell::new(AppState::new(db)));
    let ui = MainWindow::new().context("failed to construct main window")?;

    wire_callbacks(&ui, state.clone());
    refresh_ui(&ui, &state.borrow())?;
    ui.run().context("slint runtime error")?;
    Ok(())
}

fn wire_callbacks(ui: &MainWindow, state: Rc<RefCell<AppState>>) {
    let weak = ui.as_weak();
    let state_for_save = state.clone();
    ui.on_save_exercise(
        move |name, muscle_group, equipment, description, is_timed, is_bodyweight| {
            let outcome = (|| -> Result<()> {
                let input = ExerciseInput {
                    id: None,
                    name: name.trim().to_string(),
                    muscle_group: MuscleGroup::from_label(muscle_group.as_str()),
                    equipment: Equipment::from_label(equipment.as_str()),
                    description: description.trim().to_string(),
                    is_timed,
                    is_bodyweight,
                };
                let mut state = state_for_save.borrow_mut();
                state.db.save_exercise(&input)?;
                state.status_message = format!("Saved exercise {}.", input.name);
                Ok(())
            })();
            with_refresh(&weak, &state_for_save, outcome);
        },
    );

    let weak = ui.as_weak();
    let state_for_delete = state.clone();
    ui.on_delete_exercise(move |exercise_id| {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_delete.borrow_mut();
            state.db.delete_exercise(i64::from(exercise_id))?;
            state.status_message = "Exercise removed.".into();
            Ok(())
        })();
        with_refresh(&weak, &state_for_delete, outcome);
    });

    let weak = ui.as_weak();
    let state_for_add = state.clone();
    ui.on_add_draft_exercise(
        move |exercise_id, set_type, sets_count, reps, duration, weight, rest, weight_type| {
            let outcome = (|| -> Result<()> {
                let mut state = state_for_add.borrow_mut();
                let exercise = state
                    .db
                    .list_exercises()?
                    .into_iter()
                    .find(|entry| entry.id == i64::from(exercise_id))
                    .context("exercise not found")?;

                let sets_count = parse_i32(&sets_count, 3)?;
                let reps = parse_optional_i32(&reps)?;
                let duration = parse_optional_i32(&duration)?;
                let weight = parse_optional_f32(&weight)?;
                let rest_seconds = parse_i32(&rest, 90)?;

                state.template_draft.exercises.push(TemplateDraftExercise {
                    exercise_id: exercise.id,
                    exercise_name: exercise.name.clone(),
                    set_type: SetType::from_label(set_type.as_str()),
                    sets_count,
                    reps,
                    duration_seconds: duration,
                    weight,
                    rest_seconds,
                    weight_type: parse_weight_type(weight_type.as_str()),
                });
                state.status_message = format!("Queued {} in template draft.", exercise.name);
                Ok(())
            })();
            with_refresh(&weak, &state_for_add, outcome);
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
            state.status_message = "Removed exercise from draft.".into();
            Ok(())
        })();
        with_refresh(&weak, &state_for_remove_draft, outcome);
    });

    let weak = ui.as_weak();
    let state_for_clear_draft = state.clone();
    ui.on_clear_draft(move || {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_clear_draft.borrow_mut();
            state.template_draft = TemplateDraft::default();
            state.status_message = "Template draft cleared.".into();
            Ok(())
        })();
        with_refresh(&weak, &state_for_clear_draft, outcome);
    });

    let weak = ui.as_weak();
    let state_for_save_template = state.clone();
    ui.on_save_template(move |name, assigned_days| {
        let outcome = (|| -> Result<()> {
            let mut state = state_for_save_template.borrow_mut();
            state.template_draft.name = name.trim().to_string();
            state.template_draft.assigned_days = normalize_days(assigned_days.as_str());
            let saved_name = state.template_draft.name.clone();
            state.db.save_template(&state.template_draft)?;
            state.template_draft = TemplateDraft::default();
            state.status_message = format!("Saved template {}.", saved_name);
            Ok(())
        })();
        with_refresh(&weak, &state_for_save_template, outcome);
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

            let exercises = template
                .exercises
                .iter()
                .map(|exercise| {
                    let description = state.db.exercise_description(exercise.exercise_id)?;
                    let sets = exercise
                        .planned_sets
                        .iter()
                        .map(|set| {
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
                                reference_label: state
                                    .db
                                    .last_reference_label(exercise.exercise_id, set.set_number)?,
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
                started_at: Local::now(),
                exercises,
            });
            state.selected_exercise_index = 0;
            state.status_message = format!("Started workout {}.", template.name);
            Ok(())
        })();
        with_refresh(&weak, &state_for_start, outcome);
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
        with_refresh(&weak, &state_for_select, outcome);
    });

    let weak = ui.as_weak();
    let state_for_complete = state.clone();
    ui.on_complete_set(move |exercise_index, set_index, reps, weight, duration| {
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
        with_refresh(&weak, &state_for_complete, outcome);
    });

    let weak = ui.as_weak();
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
        with_refresh(&weak, &state_for_finish, outcome);
    });
}

fn with_refresh(
    weak: &slint::Weak<MainWindow>,
    state: &Rc<RefCell<AppState>>,
    outcome: Result<()>,
) {
    {
        let mut state = state.borrow_mut();
        if let Err(error) = outcome {
            state.status_message = format!("Error: {error}");
        }
    }

    if let Some(ui) = weak.upgrade() {
        let _ = refresh_ui(&ui, &state.borrow());
    }
}

fn refresh_ui(ui: &MainWindow, state: &AppState) -> Result<()> {
    let exercises = state.db.list_exercises()?;
    let templates = state.db.list_templates()?;
    let recent_sessions = state.db.recent_sessions(5)?;
    let schedule = state.db.schedule()?;

    let exercise_rows = exercises
        .iter()
        .map(|exercise| ExerciseRow {
            id: to_i32(exercise.id),
            name: exercise.name.clone().into(),
            meta: format!(
                "{} • {} • {}",
                exercise.muscle_group.label(),
                exercise.equipment.label(),
                if exercise.is_timed {
                    "Timed"
                } else {
                    "Rep based"
                }
            )
            .into(),
            description: exercise.description.clone().into(),
            source: exercise.source.clone().into(),
        })
        .collect::<Vec<_>>();
    ui.set_exercises(ModelRc::new(VecModel::from(exercise_rows)));

    let template_rows = templates
        .iter()
        .map(|template| TemplateCard {
            id: to_i32(template.id),
            name: template.name.clone().into(),
            days: if template.assigned_days.is_empty() {
                SharedString::from("Free training")
            } else {
                SharedString::from(template.assigned_days.join(", "))
            },
            info: format!("{} exercises", template.exercises.len()).into(),
            created_at: template.created_at.clone().into(),
        })
        .collect::<Vec<_>>();
    ui.set_templates(ModelRc::new(VecModel::from(template_rows)));

    let session_rows = recent_sessions
        .iter()
        .map(|session| SessionCard {
            title: session.template_name.clone().into(),
            subtitle: format!(
                "{} • {}",
                format_relative_day(&session.finished_at),
                format_duration(session.duration_seconds)
            )
            .into(),
            metric: format!("{:.0} kg • {} PRs", session.total_volume, session.pr_count).into(),
        })
        .collect::<Vec<_>>();
    ui.set_recent_sessions(ModelRc::new(VecModel::from(session_rows)));

    let schedule_rows = schedule
        .iter()
        .map(|entry| ScheduleCard {
            day: entry.day.clone().into(),
            title: entry.workout_name.clone().into(),
            detail: entry.detail.clone().into(),
        })
        .collect::<Vec<_>>();
    ui.set_schedule(ModelRc::new(VecModel::from(schedule_rows)));

    let draft_rows = state
        .template_draft
        .exercises
        .iter()
        .enumerate()
        .map(|(index, draft)| DraftExerciseRow {
            index: index as i32,
            title: draft.exercise_name.clone().into(),
            detail: format!(
                "{} x {} • {} • rest {}",
                draft.sets_count,
                match draft.set_type {
                    SetType::Timed => draft
                        .duration_seconds
                        .map(|value| format!("{}s", value))
                        .unwrap_or_else(|| "timed".into()),
                    _ => draft
                        .reps
                        .map(|value| format!("{} reps", value))
                        .unwrap_or_else(|| "auto reps".into()),
                },
                match draft.weight {
                    Some(value) => format!("{value:.1} {}", draft.weight_type.label()),
                    None => draft.weight_type.label().into(),
                },
                format_duration(draft.rest_seconds)
            )
            .into(),
        })
        .collect::<Vec<_>>();
    ui.set_draft_exercises(ModelRc::new(VecModel::from(draft_rows)));

    ui.set_status_message(state.status_message.clone().into());
    ui.set_home_hero_title("Built for structured gym progression".into());
    ui.set_home_hero_subtitle(
        "The MVP covers exercise CRUD, reusable workout templates, active set logging, session history, and visible PR feedback.".into(),
    );
    ui.set_next_workout_label(next_workout_label(&schedule));
    ui.set_streak_label(streak_label(&recent_sessions));

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
                exercise
                    .sets
                    .iter()
                    .enumerate()
                    .map(|(set_index, set)| WorkoutSetRow {
                        exercise_index: selected_index as i32,
                        set_index: set_index as i32,
                        title: format!("Set {}", set.set_number).into(),
                        plan: format_set_plan(set).into(),
                        actual: format_set_actual(set).into(),
                        reference: set.reference_label.clone().into(),
                        status: if set.completed && set.is_pr {
                            SharedString::from("PR")
                        } else if set.completed {
                            SharedString::from("Done")
                        } else {
                            SharedString::from("Open")
                        },
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

    Ok(())
}

fn parse_i32(text: &str, fallback: i32) -> Result<i32> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(fallback);
    }
    Ok(trimmed.parse::<i32>()?)
}

fn parse_optional_i32(text: &str) -> Result<Option<i32>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(trimmed.parse::<i32>()?))
}

fn parse_optional_f32(text: &str) -> Result<Option<f32>> {
    let trimmed = text.trim().replace(',', ".");
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(trimmed.parse::<f32>()?))
}

fn parse_weight_type(value: &str) -> WeightType {
    match value.trim() {
        "Lbs" => WeightType::Lbs,
        "Bodyweight" => WeightType::Bodyweight,
        "Assisted" => WeightType::Assisted,
        "BW+" => WeightType::BwPlus,
        _ => WeightType::Kg,
    }
}

fn to_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn next_workout_label(schedule: &[models::ScheduleEntry]) -> SharedString {
    schedule
        .first()
        .map(|entry| format!("{} - {}", entry.day, entry.workout_name).into())
        .unwrap_or_else(|| SharedString::from("No workout scheduled yet"))
}

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
