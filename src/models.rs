use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};

/// Represents the main muscle groups targeted by exercises.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MuscleGroup {
    UpperBack,
    LowerBack,
    Chest,
    Core,
    Arms,
    Legs,
    Shoulders,
    Glutes,
    Cardio,
}

impl MuscleGroup {
    /// Returns a human-readable label for the muscle group to be displayed in the UI.
    pub fn label(&self) -> &'static str {
        match self {
            Self::UpperBack => "Upper Back",
            Self::LowerBack => "Lower Back",
            Self::Chest => "Chest",
            Self::Core => "Core",
            Self::Arms => "Arms",
            Self::Legs => "Legs",
            Self::Shoulders => "Shoulders",
            Self::Glutes => "Glutes",
            Self::Cardio => "Cardio",
        }
    }

    /// Parses a string into a `MuscleGroup`, defaulting to `Chest` if the string doesn't match a known group.
    pub fn from_label(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "upper back" | "upperback" => Self::UpperBack,
            "lower back" | "lowerback" => Self::LowerBack,
            "core" => Self::Core,
            "arms" => Self::Arms,
            "legs" => Self::Legs,
            "shoulders" => Self::Shoulders,
            "glutes" => Self::Glutes,
            "cardio" => Self::Cardio,
            _ => Self::Chest,
        }
    }
}

/// Represents the type of equipment required for an exercise.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Equipment {
    Barbell,
    Dumbbell,
    Machine,
    Cable,
    Bodyweight,
    ResistanceBand,
    None,
}

impl Equipment {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Barbell => "Barbell",
            Self::Dumbbell => "Dumbbell",
            Self::Machine => "Machine",
            Self::Cable => "Cable",
            Self::Bodyweight => "Bodyweight",
            Self::ResistanceBand => "Resistance Band",
            Self::None => "None",
        }
    }

    pub fn from_label(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "barbell" => Self::Barbell,
            "dumbbell" => Self::Dumbbell,
            "machine" => Self::Machine,
            "cable" => Self::Cable,
            "bodyweight" => Self::Bodyweight,
            "resistance band" | "resistanceband" => Self::ResistanceBand,
            _ => Self::None,
        }
    }
}

/// Defines the type of set (e.g., standard, to failure, warmup) to customize tracking behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SetType {
    Normal,
    ToFailure,
    Timed,
    Dropset,
    Warmup,
}

impl SetType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::ToFailure => "To Failure",
            Self::Timed => "Timed",
            Self::Dropset => "Dropset",
            Self::Warmup => "Warmup",
        }
    }

    pub fn from_label(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "to failure" | "tofailure" => Self::ToFailure,
            "timed" => Self::Timed,
            "dropset" => Self::Dropset,
            "warmup" => Self::Warmup,
            _ => Self::Normal,
        }
    }
}

/// Specifies the type of weight being used (e.g., Kilograms, Pounds, or Bodyweight).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WeightType {
    Kg,
    Lbs,
    Bodyweight,
    Assisted,
    BwPlus,
}

impl WeightType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Kg => "Kg",
            Self::Lbs => "Lbs",
            Self::Bodyweight => "Bodyweight",
            Self::Assisted => "Assisted",
            Self::BwPlus => "BW+",
        }
    }
}

/// Identifies different types of Personal Records (PRs) that can be tracked.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecordType {
    MaxWeight,
    Max1Rm,
    MaxReps,
    MaxDuration,
}

impl RecordType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::MaxWeight => "Max Weight",
            Self::Max1Rm => "Max 1RM",
            Self::MaxReps => "Max Reps",
            Self::MaxDuration => "Max Duration",
        }
    }
}

/// Represents a single exercise in the database, containing its metadata and instructions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exercise {
    pub id: i64,
    pub name: String,
    pub muscle_group: MuscleGroup,
    pub equipment: Equipment,
    pub description: String,
    pub image_path: Option<String>,
    pub is_timed: bool,
    pub is_bodyweight: bool,
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OptimizedExercise {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub _filter_muscle: String,
    pub _filter_equipment: String,
}

/// Represents a planned set within a workout template, describing what the user intends to do.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedSet {
    pub set_number: i32,
    pub set_type: SetType,
    pub reps: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub weight: Option<f32>,
    pub weight_type: WeightType,
    pub rest_seconds: i32,
}

/// An exercise included in a workout template, containing multiple planned sets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateExercise {
    pub id: i64,
    pub exercise_id: i64,
    pub exercise_name: String,
    pub order_index: i32,
    pub planned_sets: Vec<PlannedSet>,
}

/// A complete workout template created by the user, defining a reusable routine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutTemplate {
    pub id: i64,
    pub name: String,
    pub icon: String,
    pub created_at: String,
    pub assigned_days: Vec<String>,
    pub exercises: Vec<TemplateExercise>,
}

/// An ongoing, draft set being created during the workout planning phase.
#[derive(Debug, Clone)]
pub struct DraftSet {
    pub reps: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub weight: Option<f32>,
    pub rest_seconds: i32,
}

/// An exercise temporarily added to a draft template during creation or editing.
#[derive(Debug, Clone)]
pub struct TemplateDraftExercise {
    pub exercise_id: i64,
    pub exercise_name: String,
    pub set_type: SetType,
    pub weight_type: WeightType,
    pub sets: Vec<DraftSet>,
}

/// The current state of a template being actively edited or created before saving.
#[derive(Debug, Clone)]
pub struct TemplateDraft {
    pub id: Option<i64>,
    pub name: String,
    pub icon: String,
    pub assigned_days: Vec<String>,
    pub exercises: Vec<TemplateDraftExercise>,
}

impl Default for TemplateDraft {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            icon: String::from("chest"),
            assigned_days: Vec::new(),
            exercises: Vec::new(),
        }
    }
}

/// Represents a set being performed during an active workout session.
/// Tracks both planned targets and actual achieved results.
#[derive(Debug, Clone)]
pub struct ActiveSet {
    pub set_number: i32,
    pub set_type: SetType,
    pub planned_reps: Option<i32>,
    pub planned_weight: Option<f32>,
    pub planned_duration: Option<i32>,
    pub weight_type: WeightType,
    pub rest_seconds: i32,
    pub actual_reps: Option<i32>,
    pub actual_weight: Option<f32>,
    pub actual_duration: Option<i32>,
    pub completed: bool,
    pub is_pr: bool,
    pub reference_label: String,
}

/// Represents an exercise currently being performed during an active workout session.
#[derive(Debug, Clone)]
pub struct ActiveExercise {
    pub exercise_id: i64,
    pub name: String,
    pub description: String,
    pub sets: Vec<ActiveSet>,
}

/// The complete state of an ongoing workout session.
#[derive(Debug, Clone)]
pub struct ActiveWorkout {
    pub template_id: i64,
    pub template_name: String,
    pub icon: String,
    pub started_at: DateTime<Local>,
    pub exercises: Vec<ActiveExercise>,
}

/// A summary of a completed workout session, typically used for rendering the history list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutSessionSummary {
    pub id: i64,
    pub template_name: String,
    pub icon: String,
    pub started_at: String,
    pub finished_at: String,
    pub duration_seconds: i32,
    pub total_volume: f32,
    pub pr_count: i32,
}

#[derive(Debug, Clone)]
pub struct ScheduleEntry {
    pub day: String,
    pub workout_name: String,
    pub detail: String,
}

/// Represents a logged Personal Record (PR) for a specific exercise.
#[derive(Debug, Clone)]
pub struct PersonalRecord {
    pub exercise_id: i64,
    pub record_type: RecordType,
    pub value: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppExportBundle {
    pub schema_version: i32,
    pub app_version: String,
    pub exported_at: String,
    pub settings: crate::settings::AppSettings,
    pub exercises: Vec<ExerciseRecord>,
    pub workout_templates: Vec<WorkoutTemplateRecord>,
    pub template_exercises: Vec<TemplateExerciseRecord>,
    pub planned_sets: Vec<PlannedSetRecord>,
    pub workout_sessions: Vec<WorkoutSessionRecord>,
    pub session_sets: Vec<SessionSetRecord>,
    pub personal_records: Vec<PersonalRecordRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExerciseRecord {
    pub id: i64,
    pub name: String,
    pub muscle_group: String,
    pub equipment: String,
    pub description: String,
    pub image_path: Option<String>,
    pub is_timed: i32,
    pub is_bodyweight: i32,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutTemplateRecord {
    pub id: i64,
    pub name: String,
    pub icon: String,
    pub assigned_days: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateExerciseRecord {
    pub id: i64,
    pub template_id: i64,
    pub exercise_id: i64,
    pub order_index: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedSetRecord {
    pub id: i64,
    pub template_exercise_id: i64,
    pub set_number: i32,
    pub set_type: String,
    pub reps: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub weight: Option<f32>,
    pub weight_type: String,
    pub rest_seconds: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutSessionRecord {
    pub id: i64,
    pub template_id: Option<i64>,
    pub template_name: String,
    pub icon: String,
    pub started_at: String,
    pub finished_at: String,
    pub duration_seconds: i32,
    pub total_volume: f32,
    pub pr_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSetRecord {
    pub id: i64,
    pub session_id: i64,
    pub exercise_id: i64,
    pub exercise_name: String,
    pub set_number: i32,
    pub set_type: String,
    pub reps_actual: Option<i32>,
    pub weight_actual: Option<f32>,
    pub weight_type: String,
    pub duration_actual: Option<i32>,
    pub completed: i32,
    pub is_pr: i32,
    pub rest_seconds: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalRecordRecord {
    pub id: i64,
    pub exercise_id: i64,
    pub record_type: String,
    pub value: f32,
    pub achieved_at: String,
    pub session_id: i64,
}

/// Normalizes a comma-separated string of days into a vector of properly capitalized strings.
pub fn normalize_days(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(capitalize_day)
        .collect()
}

/// Ensures a day string is correctly capitalized (e.g. "mon" -> "Monday").
pub fn capitalize_day(value: &str) -> String {
    let lower = value.trim().to_ascii_lowercase();
    match lower.as_str() {
        "mon" | "monday" => "Monday".into(),
        "tue" | "tuesday" => "Tuesday".into(),
        "wed" | "wednesday" => "Wednesday".into(),
        "thu" | "thursday" => "Thursday".into(),
        "fri" | "friday" => "Friday".into(),
        "sat" | "saturday" => "Saturday".into(),
        "sun" | "sunday" => "Sunday".into(),
        _ => {
            let mut chars = lower.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        }
    }
}

/// Returns the current local time formatted as a string (YYYY-MM-DD HH:MM).
pub fn now_stamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M").to_string()
}

/// Formats an integer amount of seconds into a human-readable duration string (e.g. "1m 30s").
pub fn format_duration(seconds: i32) -> String {
    let minutes = seconds / 60;
    let remainder = seconds % 60;
    if minutes > 0 {
        format!("{}m {}s", minutes, remainder)
    } else {
        format!("{}s", remainder)
    }
}

/// Formats an optional float, removing the fractional part if it's zero, or replacing None with a dash.
pub fn format_optional_number(value: Option<f32>) -> String {
    match value {
        Some(number) if (number.fract() - 0.0).abs() < f32::EPSILON => format!("{number:.0}"),
        Some(number) => format!("{number:.1}"),
        None => "-".into(),
    }
}

/// Formats the planned goal of a given `ActiveSet` for display in the UI.
pub fn format_set_plan(set: &ActiveSet) -> String {
    if let Some(duration) = set.planned_duration {
        format!("{} for {}", set.set_type.label(), format_duration(duration))
    } else {
        let reps = set
            .planned_reps
            .map(|value| value.to_string())
            .unwrap_or_else(|| "auto".into());
        let weight = set
            .planned_weight
            .map(|value| {
                format!(
                    "{} {}",
                    format_optional_number(Some(value)),
                    set.weight_type.label()
                )
            })
            .unwrap_or_else(|| set.weight_type.label().to_string());
        format!("{} reps / {}", reps, weight)
    }
}

/// Formats the actually achieved (completed) metrics of an `ActiveSet` for UI display.
pub fn format_set_actual(set: &ActiveSet) -> String {
    if !set.completed {
        return "Open".into();
    }
    if let Some(duration) = set.actual_duration {
        return format!("Done in {}", format_duration(duration));
    }
    let reps = set
        .actual_reps
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".into());
    let weight = set
        .actual_weight
        .map(|value| {
            format!(
                "{} {}",
                format_optional_number(Some(value)),
                set.weight_type.label()
            )
        })
        .unwrap_or_else(|| set.weight_type.label().to_string());
    format!("{} reps / {}", reps, weight)
}

/// Formats a date string into a relative format like "today" or "2 days ago" if recent.
pub fn format_relative_day(stamp: &str) -> String {
    if let Ok(parsed_naive) = NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d %H:%M") {
        let Some(parsed) = Local.from_local_datetime(&parsed_naive).single() else {
            return stamp.to_string();
        };
        let now = Local::now();
        let diff = now.date_naive() - parsed.date_naive();
        match diff.num_days() {
            0 => "today".into(),
            1 => "1 day ago".into(),
            days if days > 1 => format!("{} days ago", days),
            _ => parsed.format("%d.%m").to_string(),
        }
    } else {
        stamp.to_string()
    }
}

/// Generates a list of weekday names starting from the current day.
pub fn weekday_order() -> Vec<String> {
    let today = Local::now().weekday().num_days_from_monday() as usize;
    let weekdays = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    (0..7)
        .map(|offset| weekdays[(today + offset) % weekdays.len()].to_string())
        .collect()
}
