use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone)]
pub struct ExerciseInput {
    pub id: Option<i64>,
    pub name: String,
    pub muscle_group: MuscleGroup,
    pub equipment: Equipment,
    pub description: String,
    pub is_timed: bool,
    pub is_bodyweight: bool,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateExercise {
    pub id: i64,
    pub exercise_id: i64,
    pub exercise_name: String,
    pub order_index: i32,
    pub planned_sets: Vec<PlannedSet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutTemplate {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub assigned_days: Vec<String>,
    pub exercises: Vec<TemplateExercise>,
}

#[derive(Debug, Clone)]
pub struct TemplateDraftExercise {
    pub exercise_id: i64,
    pub exercise_name: String,
    pub set_type: SetType,
    pub sets_count: i32,
    pub reps: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub weight: Option<f32>,
    pub rest_seconds: i32,
    pub weight_type: WeightType,
}

#[derive(Debug, Clone)]
pub struct TemplateDraft {
    pub name: String,
    pub assigned_days: Vec<String>,
    pub exercises: Vec<TemplateDraftExercise>,
}

impl Default for TemplateDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            assigned_days: Vec::new(),
            exercises: Vec::new(),
        }
    }
}

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

#[derive(Debug, Clone)]
pub struct ActiveExercise {
    pub exercise_id: i64,
    pub name: String,
    pub description: String,
    pub sets: Vec<ActiveSet>,
}

#[derive(Debug, Clone)]
pub struct ActiveWorkout {
    pub template_id: i64,
    pub template_name: String,
    pub started_at: DateTime<Local>,
    pub exercises: Vec<ActiveExercise>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutSessionSummary {
    pub id: i64,
    pub template_name: String,
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

#[derive(Debug, Clone)]
pub struct PersonalRecord {
    pub exercise_id: i64,
    pub record_type: RecordType,
    pub value: f32,
}

pub fn normalize_days(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(capitalize_day)
        .collect()
}

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

pub fn now_stamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M").to_string()
}

pub fn format_duration(seconds: i32) -> String {
    let minutes = seconds / 60;
    let remainder = seconds % 60;
    if minutes > 0 {
        format!("{}m {}s", minutes, remainder)
    } else {
        format!("{}s", remainder)
    }
}

pub fn format_optional_number(value: Option<f32>) -> String {
    match value {
        Some(number) if (number.fract() - 0.0).abs() < f32::EPSILON => format!("{number:.0}"),
        Some(number) => format!("{number:.1}"),
        None => "-".into(),
    }
}

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
