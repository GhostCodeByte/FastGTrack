use std::{cell::RefCell, path::Path};

use anyhow::{Context, Result, bail};
use chrono::Local;
use rusqlite::{Connection, OptionalExtension, params};

use crate::models::{
    ActiveWorkout, Equipment, Exercise, ExerciseInput, MuscleGroup, PersonalRecord, PlannedSet,
    RecordType, ScheduleEntry, SetType, TemplateDraft, TemplateExercise, WeightType,
    WorkoutSessionSummary, WorkoutTemplate, now_stamp, weekday_order,
};

pub struct Database {
    conn: RefCell<Connection>,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path).context("failed to open database")?;
        let db = Self {
            conn: RefCell::new(conn),
        };
        db.init_schema()?;
        db.seed_starter_data()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.borrow().execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS exercises (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                muscle_group TEXT NOT NULL,
                equipment TEXT NOT NULL,
                description TEXT NOT NULL,
                image_path TEXT,
                is_timed INTEGER NOT NULL DEFAULT 0,
                is_bodyweight INTEGER NOT NULL DEFAULT 0,
                source TEXT NOT NULL DEFAULT 'user',
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS workout_templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                assigned_days TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS template_exercises (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                template_id INTEGER NOT NULL,
                exercise_id INTEGER NOT NULL,
                order_index INTEGER NOT NULL,
                FOREIGN KEY(template_id) REFERENCES workout_templates(id) ON DELETE CASCADE,
                FOREIGN KEY(exercise_id) REFERENCES exercises(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS planned_sets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                template_exercise_id INTEGER NOT NULL,
                set_number INTEGER NOT NULL,
                set_type TEXT NOT NULL,
                reps INTEGER,
                duration_seconds INTEGER,
                weight REAL,
                weight_type TEXT NOT NULL,
                rest_seconds INTEGER NOT NULL,
                FOREIGN KEY(template_exercise_id) REFERENCES template_exercises(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS workout_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                template_id INTEGER,
                template_name TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NOT NULL,
                duration_seconds INTEGER NOT NULL,
                total_volume REAL NOT NULL,
                pr_count INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY(template_id) REFERENCES workout_templates(id)
            );

            CREATE TABLE IF NOT EXISTS session_sets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL,
                exercise_id INTEGER NOT NULL,
                exercise_name TEXT NOT NULL,
                set_number INTEGER NOT NULL,
                set_type TEXT NOT NULL,
                reps_actual INTEGER,
                weight_actual REAL,
                weight_type TEXT NOT NULL,
                duration_actual INTEGER,
                completed INTEGER NOT NULL,
                is_pr INTEGER NOT NULL,
                rest_seconds INTEGER NOT NULL,
                FOREIGN KEY(session_id) REFERENCES workout_sessions(id) ON DELETE CASCADE,
                FOREIGN KEY(exercise_id) REFERENCES exercises(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS personal_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                exercise_id INTEGER NOT NULL,
                record_type TEXT NOT NULL,
                value REAL NOT NULL,
                achieved_at TEXT NOT NULL,
                session_id INTEGER NOT NULL,
                FOREIGN KEY(exercise_id) REFERENCES exercises(id) ON DELETE CASCADE,
                FOREIGN KEY(session_id) REFERENCES workout_sessions(id) ON DELETE CASCADE
            );
            "#,
        )?;
        Ok(())
    }

    fn seed_starter_data(&self) -> Result<()> {
        let count: i64 =
            self.conn
                .borrow()
                .query_row("SELECT COUNT(*) FROM exercises", [], |row| row.get(0))?;
        if count > 0 {
            return Ok(());
        }

        let starters = [
            (
                "Bench Press",
                MuscleGroup::Chest,
                Equipment::Barbell,
                "Classic horizontal press for chest, triceps and front delts.",
                false,
                false,
            ),
            (
                "Barbell Row",
                MuscleGroup::UpperBack,
                Equipment::Barbell,
                "Heavy pull for upper back thickness and grip.",
                false,
                false,
            ),
            (
                "Back Squat",
                MuscleGroup::Legs,
                Equipment::Barbell,
                "Primary lower-body strength lift with full body tension.",
                false,
                false,
            ),
            (
                "Plank",
                MuscleGroup::Core,
                Equipment::Bodyweight,
                "Timed anti-extension hold for trunk stiffness.",
                true,
                true,
            ),
            (
                "Pull-Up",
                MuscleGroup::UpperBack,
                Equipment::Bodyweight,
                "Vertical pull that can be weighted or assisted.",
                false,
                true,
            ),
        ];

        for starter in starters {
            self.conn.borrow().execute(
                "INSERT INTO exercises (name, muscle_group, equipment, description, image_path, is_timed, is_bodyweight, source, created_at)
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, 'starter', ?7)",
                params![
                    starter.0,
                    starter.1.label(),
                    starter.2.label(),
                    starter.3,
                    starter.4 as i32,
                    starter.5 as i32,
                    now_stamp()
                ],
            )?;
        }

        Ok(())
    }

    pub fn list_exercises(&self) -> Result<Vec<Exercise>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT id, name, muscle_group, equipment, description, image_path, is_timed, is_bodyweight, source
             FROM exercises
             ORDER BY source DESC, name ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Exercise {
                id: row.get(0)?,
                name: row.get(1)?,
                muscle_group: MuscleGroup::from_label(&row.get::<_, String>(2)?),
                equipment: Equipment::from_label(&row.get::<_, String>(3)?),
                description: row.get(4)?,
                image_path: row.get(5)?,
                is_timed: row.get::<_, i32>(6)? != 0,
                is_bodyweight: row.get::<_, i32>(7)? != 0,
                source: row.get(8)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn save_exercise(&self, input: &ExerciseInput) -> Result<()> {
        if input.name.trim().is_empty() {
            bail!("exercise name is required");
        }

        let name = input.name.trim();
        let description = if input.description.trim().is_empty() {
            "Custom exercise".to_string()
        } else {
            input.description.trim().to_string()
        };

        match input.id {
            Some(id) => {
                self.conn.borrow().execute(
                    "UPDATE exercises
                     SET name = ?1, muscle_group = ?2, equipment = ?3, description = ?4, is_timed = ?5, is_bodyweight = ?6
                     WHERE id = ?7",
                    params![
                        name,
                        input.muscle_group.label(),
                        input.equipment.label(),
                        description,
                        input.is_timed as i32,
                        input.is_bodyweight as i32,
                        id,
                    ],
                )?;
            }
            None => {
                self.conn.borrow().execute(
                    "INSERT INTO exercises (name, muscle_group, equipment, description, image_path, is_timed, is_bodyweight, source, created_at)
                     VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, 'user', ?7)",
                    params![
                        name,
                        input.muscle_group.label(),
                        input.equipment.label(),
                        description,
                        input.is_timed as i32,
                        input.is_bodyweight as i32,
                        now_stamp(),
                    ],
                )?;
            }
        }
        Ok(())
    }

    pub fn delete_exercise(&self, exercise_id: i64) -> Result<()> {
        self.conn
            .borrow()
            .execute("DELETE FROM exercises WHERE id = ?1", params![exercise_id])?;
        Ok(())
    }

    pub fn save_template(&self, draft: &TemplateDraft) -> Result<i64> {
        if draft.name.trim().is_empty() {
            bail!("template name is required");
        }
        if draft.exercises.is_empty() {
            bail!("template needs at least one exercise");
        }

        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO workout_templates (name, assigned_days, created_at) VALUES (?1, ?2, ?3)",
            params![
                draft.name.trim(),
                draft.assigned_days.join(", "),
                now_stamp()
            ],
        )?;
        let template_id = tx.last_insert_rowid();

        for (index, item) in draft.exercises.iter().enumerate() {
            tx.execute(
                "INSERT INTO template_exercises (template_id, exercise_id, order_index) VALUES (?1, ?2, ?3)",
                params![template_id, item.exercise_id, index as i32],
            )?;
            let template_exercise_id = tx.last_insert_rowid();
            for set_number in 0..item.sets_count {
                tx.execute(
                    "INSERT INTO planned_sets (template_exercise_id, set_number, set_type, reps, duration_seconds, weight, weight_type, rest_seconds)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        template_exercise_id,
                        set_number + 1,
                        item.set_type.label(),
                        item.reps,
                        item.duration_seconds,
                        item.weight,
                        item.weight_type.label(),
                        item.rest_seconds,
                    ],
                )?;
            }
        }

        tx.commit()?;
        Ok(template_id)
    }

    pub fn list_templates(&self) -> Result<Vec<WorkoutTemplate>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT id, name, created_at, assigned_days
             FROM workout_templates
             ORDER BY created_at DESC, name ASC",
        )?;
        let template_rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        let mut templates = Vec::new();
        for row in template_rows {
            let (id, name, created_at, assigned_days) = row?;
            templates.push(WorkoutTemplate {
                id,
                name,
                created_at,
                assigned_days: assigned_days
                    .split(',')
                    .map(str::trim)
                    .filter(|day| !day.is_empty())
                    .map(str::to_string)
                    .collect(),
                exercises: self.template_exercises(id)?,
            });
        }

        Ok(templates)
    }

    fn template_exercises(&self, template_id: i64) -> Result<Vec<TemplateExercise>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT te.id, te.exercise_id, e.name, te.order_index
             FROM template_exercises te
             JOIN exercises e ON e.id = te.exercise_id
             WHERE te.template_id = ?1
             ORDER BY te.order_index ASC",
        )?;
        let rows = stmt.query_map(params![template_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i32>(3)?,
            ))
        })?;

        let mut exercises = Vec::new();
        for row in rows {
            let (id, exercise_id, exercise_name, order_index) = row?;
            exercises.push(TemplateExercise {
                id,
                exercise_id,
                exercise_name,
                order_index,
                planned_sets: self.planned_sets(id)?,
            });
        }
        Ok(exercises)
    }

    fn planned_sets(&self, template_exercise_id: i64) -> Result<Vec<PlannedSet>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT set_number, set_type, reps, duration_seconds, weight, weight_type, rest_seconds
             FROM planned_sets
             WHERE template_exercise_id = ?1
             ORDER BY set_number ASC",
        )?;
        let rows = stmt.query_map(params![template_exercise_id], |row| {
            Ok(PlannedSet {
                set_number: row.get(0)?,
                set_type: SetType::from_label(&row.get::<_, String>(1)?),
                reps: row.get(2)?,
                duration_seconds: row.get(3)?,
                weight: row.get(4)?,
                weight_type: match row.get::<_, String>(5)?.as_str() {
                    "Lbs" => WeightType::Lbs,
                    "Bodyweight" => WeightType::Bodyweight,
                    "Assisted" => WeightType::Assisted,
                    "BW+" => WeightType::BwPlus,
                    _ => WeightType::Kg,
                },
                rest_seconds: row.get(6)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn recent_sessions(&self, limit: usize) -> Result<Vec<WorkoutSessionSummary>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT id, template_name, started_at, finished_at, duration_seconds, total_volume, pr_count
             FROM workout_sessions
             ORDER BY finished_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(WorkoutSessionSummary {
                id: row.get(0)?,
                template_name: row.get(1)?,
                started_at: row.get(2)?,
                finished_at: row.get(3)?,
                duration_seconds: row.get(4)?,
                total_volume: row.get(5)?,
                pr_count: row.get(6)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn schedule(&self) -> Result<Vec<ScheduleEntry>> {
        let templates = self.list_templates()?;
        let weekdays = weekday_order();
        let mut schedule = Vec::new();

        for day in weekdays {
            for template in templates.iter().filter(|template| {
                template
                    .assigned_days
                    .iter()
                    .any(|assigned| assigned.eq_ignore_ascii_case(&day))
            }) {
                schedule.push(ScheduleEntry {
                    day: day.clone(),
                    workout_name: template.name.clone(),
                    detail: format!("{} exercises", template.exercises.len()),
                });
            }
        }

        Ok(schedule)
    }

    pub fn load_template(&self, template_id: i64) -> Result<Option<WorkoutTemplate>> {
        Ok(self
            .list_templates()?
            .into_iter()
            .find(|template| template.id == template_id))
    }

    pub fn exercise_description(&self, exercise_id: i64) -> Result<String> {
        let description = self
            .conn
            .borrow()
            .query_row(
                "SELECT description FROM exercises WHERE id = ?1",
                params![exercise_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(description.unwrap_or_else(|| "No description yet.".into()))
    }

    pub fn last_reference_label(&self, exercise_id: i64, set_number: i32) -> Result<String> {
        let conn = self.conn.borrow();
        let row = conn
            .query_row(
                "SELECT reps_actual, weight_actual, duration_actual, weight_type
                 FROM session_sets
                 WHERE exercise_id = ?1 AND set_number = ?2 AND completed = 1
                 ORDER BY id DESC
                 LIMIT 1",
                params![exercise_id, set_number],
                |row| {
                    Ok((
                        row.get::<_, Option<i32>>(0)?,
                        row.get::<_, Option<f32>>(1)?,
                        row.get::<_, Option<i32>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;

        Ok(match row {
            Some((_, _, Some(duration), _)) => format!("Last: {}s", duration),
            Some((reps, weight, _, weight_type)) => {
                let reps = reps
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".into());
                let weight = weight
                    .map(|value| {
                        if (value.fract() - 0.0).abs() < f32::EPSILON {
                            format!("{value:.0}")
                        } else {
                            format!("{value:.1}")
                        }
                    })
                    .unwrap_or_else(|| "-".into());
                format!("Last: {} reps / {} {}", reps, weight, weight_type)
            }
            None => "Last: no data".into(),
        })
    }

    pub fn evaluate_prs(
        &self,
        exercise_id: i64,
        reps: Option<i32>,
        weight: Option<f32>,
        duration: Option<i32>,
    ) -> Result<Vec<PersonalRecord>> {
        let mut prs = Vec::new();
        if let Some(weight_value) = weight {
            let best_weight = self.best_record(exercise_id, RecordType::MaxWeight.label())?;
            if best_weight
                .map(|value| weight_value > value)
                .unwrap_or(true)
            {
                prs.push(PersonalRecord {
                    exercise_id,
                    record_type: RecordType::MaxWeight,
                    value: weight_value,
                });
            }
        }

        if let Some(reps_value) = reps {
            let best_reps = self.best_record(exercise_id, RecordType::MaxReps.label())?;
            if best_reps
                .map(|value| reps_value as f32 > value)
                .unwrap_or(true)
            {
                prs.push(PersonalRecord {
                    exercise_id,
                    record_type: RecordType::MaxReps,
                    value: reps_value as f32,
                });
            }

            if let Some(weight_value) = weight {
                let epley = weight_value * (1.0 + reps_value as f32 / 30.0);
                let best_1rm = self.best_record(exercise_id, RecordType::Max1Rm.label())?;
                if best_1rm.map(|value| epley > value).unwrap_or(true) {
                    prs.push(PersonalRecord {
                        exercise_id,
                        record_type: RecordType::Max1Rm,
                        value: epley,
                    });
                }
            }
        }

        if let Some(duration_value) = duration {
            let best_duration = self.best_record(exercise_id, RecordType::MaxDuration.label())?;
            if best_duration
                .map(|value| duration_value as f32 > value)
                .unwrap_or(true)
            {
                prs.push(PersonalRecord {
                    exercise_id,
                    record_type: RecordType::MaxDuration,
                    value: duration_value as f32,
                });
            }
        }

        Ok(prs)
    }

    fn best_record(&self, exercise_id: i64, record_type: &str) -> Result<Option<f32>> {
        self.conn
            .borrow()
            .query_row(
                "SELECT MAX(value) FROM personal_records WHERE exercise_id = ?1 AND record_type = ?2",
                params![exercise_id, record_type],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn save_completed_workout(&self, workout: &ActiveWorkout) -> Result<WorkoutSessionSummary> {
        let finished_at = Local::now();
        let duration_seconds = (finished_at - workout.started_at).num_seconds().max(0) as i32;
        let total_volume: f32 = workout
            .exercises
            .iter()
            .flat_map(|exercise| exercise.sets.iter())
            .filter_map(|set| match (set.actual_weight, set.actual_reps) {
                (Some(weight), Some(reps)) if set.completed => Some(weight * reps as f32),
                _ => None,
            })
            .sum();
        let pr_count: i32 = workout
            .exercises
            .iter()
            .flat_map(|exercise| exercise.sets.iter())
            .filter(|set| set.is_pr)
            .count() as i32;

        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO workout_sessions (template_id, template_name, started_at, finished_at, duration_seconds, total_volume, pr_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                workout.template_id,
                workout.template_name,
                workout.started_at.format("%Y-%m-%d %H:%M").to_string(),
                finished_at.format("%Y-%m-%d %H:%M").to_string(),
                duration_seconds,
                total_volume,
                pr_count,
            ],
        )?;
        let session_id = tx.last_insert_rowid();

        for exercise in &workout.exercises {
            for set in &exercise.sets {
                tx.execute(
                    "INSERT INTO session_sets (session_id, exercise_id, exercise_name, set_number, set_type, reps_actual, weight_actual, weight_type, duration_actual, completed, is_pr, rest_seconds)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        session_id,
                        exercise.exercise_id,
                        exercise.name,
                        set.set_number,
                        set.set_type.label(),
                        set.actual_reps,
                        set.actual_weight,
                        set.weight_type.label(),
                        set.actual_duration,
                        set.completed as i32,
                        set.is_pr as i32,
                        set.rest_seconds,
                    ],
                )?;

                if set.completed && set.is_pr {
                    for pr in self.evaluate_prs(
                        exercise.exercise_id,
                        set.actual_reps,
                        set.actual_weight,
                        set.actual_duration,
                    )? {
                        tx.execute(
                            "INSERT INTO personal_records (exercise_id, record_type, value, achieved_at, session_id)
                             VALUES (?1, ?2, ?3, ?4, ?5)",
                            params![
                                pr.exercise_id,
                                pr.record_type.label(),
                                pr.value,
                                finished_at.format("%Y-%m-%d %H:%M").to_string(),
                                session_id,
                            ],
                        )?;
                    }
                }
            }
        }

        tx.commit()?;

        Ok(WorkoutSessionSummary {
            id: session_id,
            template_name: workout.template_name.clone(),
            started_at: workout.started_at.format("%Y-%m-%d %H:%M").to_string(),
            finished_at: finished_at.format("%Y-%m-%d %H:%M").to_string(),
            duration_seconds,
            total_volume,
            pr_count,
        })
    }
}
