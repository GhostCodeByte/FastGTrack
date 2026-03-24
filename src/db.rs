use std::{cell::RefCell, path::Path};

use anyhow::{Context, Result, bail};
use chrono::Local;
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    models::{
        ActiveWorkout, AppExportBundle, Equipment, Exercise, ExerciseRecord, MuscleGroup,
        OptimizedExercise, PersonalRecord, PersonalRecordRecord, PlannedSet, PlannedSetRecord,
        RecordType, ScheduleEntry, SessionSetRecord, SetType, TemplateDraft, TemplateExercise,
        TemplateExerciseRecord, WeightType, WorkoutSessionRecord, WorkoutSessionSummary,
        WorkoutTemplate, WorkoutTemplateRecord, now_stamp, weekday_order,
    },
    settings::AppSettings,
};

/// Core database handler that wraps an SQLite connection to persist and retrieve app data.
pub struct Database {
    conn: RefCell<Connection>,
}

impl Database {
    /// Opens a connection to the specified SQLite database file and initializes standard configurations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path).context("failed to open database")?;
        // WAL mode + larger cache = dramatically faster reads on every query
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -8000;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 67108864;",
        )
        .context("failed to set DB PRAGMAs")?;
        let db = Self {
            conn: RefCell::new(conn),
        };
        db.init_schema()?;
        db.seed_exercises_from_json()?;
        Ok(db)
    }

    /// Creates necessary tables, indexes, and applies required schema migrations if they do not yet exist.
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
                icon TEXT NOT NULL DEFAULT 'chest',
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
                icon TEXT NOT NULL DEFAULT 'chest',
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

            -- Performance indexes
            CREATE INDEX IF NOT EXISTS idx_template_exercises_template_id
                ON template_exercises(template_id, order_index);

            CREATE INDEX IF NOT EXISTS idx_planned_sets_template_exercise_id
                ON planned_sets(template_exercise_id, set_number);

            CREATE INDEX IF NOT EXISTS idx_session_sets_exercise_set
                ON session_sets(exercise_id, set_number, completed, id);

            CREATE INDEX IF NOT EXISTS idx_personal_records_exercise_type
                ON personal_records(exercise_id, record_type, value);

            CREATE INDEX IF NOT EXISTS idx_workout_sessions_finished
                ON workout_sessions(finished_at DESC);

            CREATE INDEX IF NOT EXISTS idx_exercises_source_name
                ON exercises(source DESC, name ASC);
            "#,
        )?;

        // Migrations
        let _ = self.conn.borrow().execute(
            "ALTER TABLE workout_templates ADD COLUMN icon TEXT NOT NULL DEFAULT 'chest'",
            [],
        );
        let _ = self.conn.borrow().execute(
            "ALTER TABLE workout_sessions ADD COLUMN icon TEXT NOT NULL DEFAULT 'chest'",
            [],
        );

        Ok(())
    }

    /// Populates the database with default system exercises parsed from a bundled JSON registry.
    fn seed_exercises_from_json(&self) -> Result<()> {
        let json = include_str!("../exercises/optimized_exercises.json");
        let exercises: Vec<OptimizedExercise> = serde_json::from_str(json).unwrap_or_default();

        // Remove any user-created exercises and orphaned template data.
        {
            let conn = self.conn.borrow();
            conn.execute_batch(
                "PRAGMA foreign_keys = ON;
                 DELETE FROM exercises WHERE source = 'user' OR source = 'starter';",
            )?;
        }

        // Seed all system exercises with their real IDs (idempotent).
        for ex in &exercises {
            // Derive muscle_group and equipment from the _filter_muscle/_filter_equipment fields.
            self.conn.borrow().execute(
                "INSERT OR IGNORE INTO exercises
                     (id, name, muscle_group, equipment, description, image_path,
                      is_timed, is_bodyweight, source, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0, 0, 'system', ?6)",
                params![
                    ex.id,
                    ex.name,
                    ex._filter_muscle,
                    ex._filter_equipment,
                    ex.description,
                    now_stamp(),
                ],
            )?;
        }

        // Drop any template_exercises that reference non-existent exercises (orphans from the old
        // +1_000_000 ID scheme).  With FK ON this would already be blocked, but we do it
        // explicitly for existing bad data in the DB.
        self.conn.borrow().execute_batch(
            "DELETE FROM template_exercises
             WHERE exercise_id NOT IN (SELECT id FROM exercises);",
        )?;

        Ok(())
    }

    /// Returns a complete list of all exercises available in the database, ordered systematically.
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

    /// Saves a newly created or edited workout template into the database with its underlying exercises and sets.
    pub fn save_template(&self, draft: &TemplateDraft) -> Result<i64> {
        if draft.name.trim().is_empty() {
            bail!("template name is required");
        }
        if draft.exercises.is_empty() {
            bail!("template needs at least one exercise");
        }

        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction()?;

        let template_id = if let Some(id) = draft.id {
            tx.execute(
                "UPDATE workout_templates SET name = ?1, icon = ?2, assigned_days = ?3 WHERE id = ?4",
                params![draft.name.trim(), draft.icon.clone(), draft.assigned_days.join(", "), id],
            )?;
            tx.execute(
                "DELETE FROM template_exercises WHERE template_id = ?1",
                params![id],
            )?;
            id
        } else {
            tx.execute(
                "INSERT INTO workout_templates (name, icon, assigned_days, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![
                    draft.name.trim(),
                    draft.icon.clone(),
                    draft.assigned_days.join(", "),
                    now_stamp()
                ],
            )?;
            tx.last_insert_rowid()
        };

        for (index, item) in draft.exercises.iter().enumerate() {
            tx.execute(
                "INSERT INTO template_exercises (template_id, exercise_id, order_index) VALUES (?1, ?2, ?3)",
                params![template_id, item.exercise_id, index as i32],
            )?;
            let template_exercise_id = tx.last_insert_rowid();
            for (set_number, set) in item.sets.iter().enumerate() {
                tx.execute(
                    "INSERT INTO planned_sets (template_exercise_id, set_number, set_type, reps, duration_seconds, weight, weight_type, rest_seconds)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        template_exercise_id,
                        set_number as i32 + 1,
                        item.set_type.label(),
                        set.reps,
                        set.duration_seconds,
                        set.weight,
                        item.weight_type.label(),
                        set.rest_seconds,
                    ],
                )?;
            }
        }

        tx.commit()?;
        Ok(template_id)
    }

    /// Retrieves all workout templates, including their attached exercises and fully reconstructed planned sets.
    pub fn list_templates(&self) -> Result<Vec<WorkoutTemplate>> {
        let conn = self.conn.borrow();

        // One query for all templates
        let mut stmt = conn.prepare(
            "SELECT id, name, icon, created_at, assigned_days
             FROM workout_templates
             ORDER BY created_at DESC, name ASC",
        )?;
        let mut templates: Vec<WorkoutTemplate> = stmt
            .query_map([], |row| {
                let assigned_days_raw: String = row.get(4)?;
                Ok(WorkoutTemplate {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    icon: row.get(2)?,
                    created_at: row.get(3)?,
                    assigned_days: assigned_days_raw
                        .split(',')
                        .map(str::trim)
                        .filter(|d| !d.is_empty())
                        .map(str::to_string)
                        .collect(),
                    exercises: Vec::new(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        if templates.is_empty() {
            return Ok(templates);
        }

        // One query for all template_exercises + exercise names
        let mut te_stmt = conn.prepare(
            "SELECT te.id, te.template_id, te.exercise_id, e.name, te.order_index
             FROM template_exercises te
             JOIN exercises e ON e.id = te.exercise_id
             ORDER BY te.template_id, te.order_index ASC",
        )?;
        // template_exercise_id -> (template_id, exercise entry)
        let te_rows: Vec<(i64, i64, i64, String, i32)> = te_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i32>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        // One query for all planned_sets
        let mut ps_stmt = conn.prepare(
            "SELECT ps.template_exercise_id, ps.set_number, ps.set_type,
                    ps.reps, ps.duration_seconds, ps.weight, ps.weight_type, ps.rest_seconds
             FROM planned_sets ps
             JOIN template_exercises te ON te.id = ps.template_exercise_id
             ORDER BY ps.template_exercise_id, ps.set_number ASC",
        )?;
        // group planned sets by template_exercise_id
        let mut sets_by_te: std::collections::HashMap<i64, Vec<PlannedSet>> =
            std::collections::HashMap::new();
        for row in ps_stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                PlannedSet {
                    set_number: row.get(1)?,
                    set_type: SetType::from_label(&row.get::<_, String>(2)?),
                    reps: row.get(3)?,
                    duration_seconds: row.get(4)?,
                    weight: row.get(5)?,
                    weight_type: match row.get::<_, String>(6)?.as_str() {
                        "Lbs" => WeightType::Lbs,
                        "Bodyweight" => WeightType::Bodyweight,
                        "Assisted" => WeightType::Assisted,
                        "BW+" => WeightType::BwPlus,
                        _ => WeightType::Kg,
                    },
                    rest_seconds: row.get(7)?,
                },
            ))
        })? {
            let (te_id, ps) = row?;
            sets_by_te.entry(te_id).or_default().push(ps);
        }

        // Build template_exercises per template
        let mut exercises_by_template: std::collections::HashMap<i64, Vec<TemplateExercise>> =
            std::collections::HashMap::new();
        for (te_id, template_id, exercise_id, exercise_name, order_index) in te_rows {
            let planned_sets = sets_by_te.remove(&te_id).unwrap_or_default();
            exercises_by_template
                .entry(template_id)
                .or_default()
                .push(TemplateExercise {
                    id: te_id,
                    exercise_id,
                    exercise_name,
                    order_index,
                    planned_sets,
                });
        }

        for t in &mut templates {
            if let Some(exs) = exercises_by_template.remove(&t.id) {
                t.exercises = exs;
            }
        }

        Ok(templates)
    }

    /// Fetches the list of strictly planned sets configured for a specific exercise instance inside a template.
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

    /// Gets a summarized list of the most recent completed workout sessions up to a given limit count.
    pub fn recent_sessions(&self, limit: usize) -> Result<Vec<WorkoutSessionSummary>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT id, template_name, icon, started_at, finished_at, duration_seconds, total_volume, pr_count
             FROM workout_sessions
             ORDER BY finished_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(WorkoutSessionSummary {
                id: row.get(0)?,
                template_name: row.get(1)?,
                icon: row.get(2)?,
                started_at: row.get(3)?,
                finished_at: row.get(4)?,
                duration_seconds: row.get(5)?,
                total_volume: row.get(6)?,
                pr_count: row.get(7)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// Reconstructs the weekly workout schedule sequentially based on assigned days attached to existing templates.
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

    /// Permanently deletes a template and gracefully unlinks it from any past completed sessions.
    pub fn delete_template(&self, template_id: i64) -> Result<()> {
        self.conn.borrow().execute(
            "UPDATE workout_sessions SET template_id = NULL WHERE template_id = ?1",
            params![template_id],
        )?;
        self.conn.borrow().execute(
            "DELETE FROM workout_templates WHERE id = ?1",
            params![template_id],
        )?;
        Ok(())
    }

    pub fn export_bundle(&self, settings: &AppSettings) -> Result<AppExportBundle> {
        let conn = self.conn.borrow();

        let exercises = {
            let mut stmt = conn.prepare("SELECT id, name, muscle_group, equipment, description, image_path, is_timed, is_bodyweight, source, created_at FROM exercises ORDER BY id")?;
            stmt.query_map([], |row| {
                Ok(ExerciseRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    muscle_group: row.get(2)?,
                    equipment: row.get(3)?,
                    description: row.get(4)?,
                    image_path: row.get(5)?,
                    is_timed: row.get(6)?,
                    is_bodyweight: row.get(7)?,
                    source: row.get(8)?,
                    created_at: row.get(9)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };

        let workout_templates = {
            let mut stmt = conn.prepare("SELECT id, name, icon, assigned_days, created_at FROM workout_templates ORDER BY id")?;
            stmt.query_map([], |row| {
                Ok(WorkoutTemplateRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    icon: row.get(2)?,
                    assigned_days: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };

        let template_exercises = {
            let mut stmt = conn.prepare("SELECT id, template_id, exercise_id, order_index FROM template_exercises ORDER BY id")?;
            stmt.query_map([], |row| {
                Ok(TemplateExerciseRecord {
                    id: row.get(0)?,
                    template_id: row.get(1)?,
                    exercise_id: row.get(2)?,
                    order_index: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };

        let planned_sets = {
            let mut stmt = conn.prepare("SELECT id, template_exercise_id, set_number, set_type, reps, duration_seconds, weight, weight_type, rest_seconds FROM planned_sets ORDER BY id")?;
            stmt.query_map([], |row| {
                Ok(PlannedSetRecord {
                    id: row.get(0)?,
                    template_exercise_id: row.get(1)?,
                    set_number: row.get(2)?,
                    set_type: row.get(3)?,
                    reps: row.get(4)?,
                    duration_seconds: row.get(5)?,
                    weight: row.get(6)?,
                    weight_type: row.get(7)?,
                    rest_seconds: row.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };

        let workout_sessions = {
            let mut stmt = conn.prepare("SELECT id, template_id, template_name, icon, started_at, finished_at, duration_seconds, total_volume, pr_count FROM workout_sessions ORDER BY id")?;
            stmt.query_map([], |row| {
                Ok(WorkoutSessionRecord {
                    id: row.get(0)?,
                    template_id: row.get(1)?,
                    template_name: row.get(2)?,
                    icon: row.get(3)?,
                    started_at: row.get(4)?,
                    finished_at: row.get(5)?,
                    duration_seconds: row.get(6)?,
                    total_volume: row.get(7)?,
                    pr_count: row.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };

        let session_sets = {
            let mut stmt = conn.prepare("SELECT id, session_id, exercise_id, exercise_name, set_number, set_type, reps_actual, weight_actual, weight_type, duration_actual, completed, is_pr, rest_seconds FROM session_sets ORDER BY id")?;
            stmt.query_map([], |row| {
                Ok(SessionSetRecord {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    exercise_id: row.get(2)?,
                    exercise_name: row.get(3)?,
                    set_number: row.get(4)?,
                    set_type: row.get(5)?,
                    reps_actual: row.get(6)?,
                    weight_actual: row.get(7)?,
                    weight_type: row.get(8)?,
                    duration_actual: row.get(9)?,
                    completed: row.get(10)?,
                    is_pr: row.get(11)?,
                    rest_seconds: row.get(12)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };

        let personal_records = {
            let mut stmt = conn.prepare("SELECT id, exercise_id, record_type, value, achieved_at, session_id FROM personal_records ORDER BY id")?;
            stmt.query_map([], |row| {
                Ok(PersonalRecordRecord {
                    id: row.get(0)?,
                    exercise_id: row.get(1)?,
                    record_type: row.get(2)?,
                    value: row.get(3)?,
                    achieved_at: row.get(4)?,
                    session_id: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };

        Ok(AppExportBundle {
            schema_version: 1,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            exported_at: now_stamp(),
            settings: settings.clone(),
            exercises,
            workout_templates,
            template_exercises,
            planned_sets,
            workout_sessions,
            session_sets,
            personal_records,
        })
    }

    pub fn import_bundle(&self, bundle: &AppExportBundle) -> Result<()> {
        if bundle.schema_version != 1 {
            bail!(
                "unsupported import schema version: {}",
                bundle.schema_version
            );
        }

        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction()?;
        tx.execute_batch(
            "PRAGMA foreign_keys = OFF;
             DELETE FROM personal_records;
             DELETE FROM session_sets;
             DELETE FROM workout_sessions;
             DELETE FROM planned_sets;
             DELETE FROM template_exercises;
             DELETE FROM workout_templates;
             DELETE FROM exercises;",
        )?;

        for row in &bundle.exercises {
            tx.execute(
                "INSERT INTO exercises (id, name, muscle_group, equipment, description, image_path, is_timed, is_bodyweight, source, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![row.id, row.name, row.muscle_group, row.equipment, row.description, row.image_path, row.is_timed, row.is_bodyweight, row.source, row.created_at],
            )?;
        }
        for row in &bundle.workout_templates {
            tx.execute(
                "INSERT INTO workout_templates (id, name, icon, assigned_days, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![row.id, row.name, row.icon, row.assigned_days, row.created_at],
            )?;
        }
        for row in &bundle.template_exercises {
            tx.execute(
                "INSERT INTO template_exercises (id, template_id, exercise_id, order_index) VALUES (?1, ?2, ?3, ?4)",
                params![row.id, row.template_id, row.exercise_id, row.order_index],
            )?;
        }
        for row in &bundle.planned_sets {
            tx.execute(
                "INSERT INTO planned_sets (id, template_exercise_id, set_number, set_type, reps, duration_seconds, weight, weight_type, rest_seconds) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![row.id, row.template_exercise_id, row.set_number, row.set_type, row.reps, row.duration_seconds, row.weight, row.weight_type, row.rest_seconds],
            )?;
        }
        for row in &bundle.workout_sessions {
            tx.execute(
                "INSERT INTO workout_sessions (id, template_id, template_name, icon, started_at, finished_at, duration_seconds, total_volume, pr_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![row.id, row.template_id, row.template_name, row.icon, row.started_at, row.finished_at, row.duration_seconds, row.total_volume, row.pr_count],
            )?;
        }
        for row in &bundle.session_sets {
            tx.execute(
                "INSERT INTO session_sets (id, session_id, exercise_id, exercise_name, set_number, set_type, reps_actual, weight_actual, weight_type, duration_actual, completed, is_pr, rest_seconds) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![row.id, row.session_id, row.exercise_id, row.exercise_name, row.set_number, row.set_type, row.reps_actual, row.weight_actual, row.weight_type, row.duration_actual, row.completed, row.is_pr, row.rest_seconds],
            )?;
        }
        for row in &bundle.personal_records {
            tx.execute(
                "INSERT INTO personal_records (id, exercise_id, record_type, value, achieved_at, session_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![row.id, row.exercise_id, row.record_type, row.value, row.achieved_at, row.session_id],
            )?;
        }

        tx.execute_batch("PRAGMA foreign_keys = ON;")?;
        tx.commit()?;
        Ok(())
    }

    pub fn clear_all_data(&self) -> Result<()> {
        self.conn.borrow().execute_batch(
            "DELETE FROM personal_records;
             DELETE FROM session_sets;
             DELETE FROM workout_sessions;
             DELETE FROM planned_sets;
             DELETE FROM template_exercises;
             DELETE FROM workout_templates;
             DELETE FROM exercises;",
        )?;
        Ok(())
    }

    /// Retrieves the full, detailed structure of a specific workout template (usually for editing or starting it).
    pub fn load_template(&self, template_id: i64) -> Result<Option<WorkoutTemplate>> {
        let conn = self.conn.borrow();

        let row = conn
            .query_row(
                "SELECT id, name, icon, created_at, assigned_days
                 FROM workout_templates WHERE id = ?1",
                params![template_id],
                |row| {
                    let assigned_days_raw: String = row.get(4)?;
                    Ok(WorkoutTemplate {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        icon: row.get(2)?,
                        created_at: row.get(3)?,
                        assigned_days: assigned_days_raw
                            .split(',')
                            .map(str::trim)
                            .filter(|d| !d.is_empty())
                            .map(str::to_string)
                            .collect(),
                        exercises: Vec::new(),
                    })
                },
            )
            .optional()?;

        let Some(mut template) = row else {
            return Ok(None);
        };

        // Exercises for this template only
        let mut te_stmt = conn.prepare(
            "SELECT te.id, te.exercise_id, e.name, te.order_index
             FROM template_exercises te
             JOIN exercises e ON e.id = te.exercise_id
             WHERE te.template_id = ?1
             ORDER BY te.order_index ASC",
        )?;
        let te_rows: Vec<(i64, i64, String, i32)> = te_stmt
            .query_map(params![template_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i32>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        for (te_id, exercise_id, exercise_name, order_index) in te_rows {
            let planned_sets = self.planned_sets(te_id)?;
            template.exercises.push(TemplateExercise {
                id: te_id,
                exercise_id,
                exercise_name,
                order_index,
                planned_sets,
            });
        }

        Ok(Some(template))
    }

    /// Fetch the formatted description text for a specific exercise ID.
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

    /// Fetch the last completed set reference label for every (exercise_id, set_number)
    /// pair in `exercise_ids` — one SQL query instead of one per set.
    /// Returns a HashMap keyed by (exercise_id, set_number).
    pub fn bulk_reference_labels(
        &self,
        exercise_ids: &[i64],
    ) -> Result<std::collections::HashMap<(i64, i32), String>> {
        use std::collections::HashMap;

        if exercise_ids.is_empty() {
            return Ok(HashMap::new());
        }

        // Build a parameterised IN clause.
        let placeholders = exercise_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "SELECT s.exercise_id, s.set_number,
                    s.reps_actual, s.weight_actual, s.duration_actual, s.weight_type
             FROM session_sets s
             INNER JOIN (
                 SELECT exercise_id, set_number, MAX(id) AS max_id
                 FROM session_sets
                 WHERE exercise_id IN ({placeholders}) AND completed = 1
                 GROUP BY exercise_id, set_number
             ) latest ON s.id = latest.max_id"
        );

        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(&sql)?;

        // Build the params slice dynamically.
        let params_vec: Vec<&dyn rusqlite::types::ToSql> = exercise_ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();

        let rows = stmt.query_map(params_vec.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, Option<i32>>(2)?,
                row.get::<_, Option<f32>>(3)?,
                row.get::<_, Option<i32>>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;

        let mut map = HashMap::new();
        for row in rows {
            let (exercise_id, set_number, reps, weight, duration, weight_type) = row?;
            let label = match duration {
                Some(d) => format!("Last: {}s", d),
                None => {
                    let reps_str = reps.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
                    let weight_str = weight
                        .map(|v| {
                            if (v.fract() - 0.0).abs() < f32::EPSILON {
                                format!("{v:.0}")
                            } else {
                                format!("{v:.1}")
                            }
                        })
                        .unwrap_or_else(|| "-".into());
                    format!("Last: {} reps / {} {}", reps_str, weight_str, weight_type)
                }
            };
            map.insert((exercise_id, set_number), label);
        }
        Ok(map)
    }

    /// Compares newly achieved stats against past performance to detect and internally register any new Personal Records (PRs) before rendering the summary.
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

    /// Looks up the all-time highest recorded metric value of a given distinct PR type for a specific exercise.
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

    /// Stores a freshly finished active session in the database including all its uniquely completed sets and logs newly unlocked PRs.
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
            "INSERT INTO workout_sessions (template_id, template_name, icon, started_at, finished_at, duration_seconds, total_volume, pr_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                workout.template_id,
                workout.template_name,
                workout.icon,
                workout.started_at.format("%Y-%m-%d %H:%M").to_string(),
                finished_at.format("%Y-%m-%d %H:%M").to_string(),
                duration_seconds,
                total_volume,
                pr_count
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
            icon: workout.icon.clone(),
            started_at: workout.started_at.format("%Y-%m-%d %H:%M").to_string(),
            finished_at: finished_at.format("%Y-%m-%d %H:%M").to_string(),
            duration_seconds,
            total_volume,
            pr_count,
        })
    }

    // ── Statistics queries ─────────────────────────────────────────

    /// Retrieves all workout sessions successfully completed within the last specified number of days.
    pub fn stats_sessions_in_period(&self, days: i64) -> Result<Vec<WorkoutSessionSummary>> {
        let cutoff = (Local::now() - chrono::Duration::days(days))
            .format("%Y-%m-%d %H:%M")
            .to_string();
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT id, template_name, icon, started_at, finished_at, duration_seconds, total_volume, pr_count
             FROM workout_sessions
             WHERE finished_at >= ?1
             ORDER BY finished_at DESC",
        )?;
        let rows = stmt.query_map(params![cutoff], |row| {
            Ok(WorkoutSessionSummary {
                id: row.get(0)?,
                template_name: row.get(1)?,
                icon: row.get(2)?,
                started_at: row.get(3)?,
                finished_at: row.get(4)?,
                duration_seconds: row.get(5)?,
                total_volume: row.get(6)?,
                pr_count: row.get(7)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// Calculates the total count of distinct Personal Records (PRs) achieved across all exercises within the specified time period.
    pub fn stats_pr_count_in_period(&self, days: i64) -> Result<i64> {
        let cutoff = (Local::now() - chrono::Duration::days(days))
            .format("%Y-%m-%d %H:%M")
            .to_string();
        let count: i64 = self.conn.borrow().query_row(
            "SELECT COUNT(*) FROM personal_records WHERE achieved_at >= ?1",
            params![cutoff],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Identifies and returns the specific day-of-month numbers that log at least one completed session for the given year and month.
    pub fn stats_calendar_days(&self, year: i32, month: u32) -> Result<Vec<u32>> {
        let prefix = format!("{:04}-{:02}", year, month);
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT CAST(SUBSTR(finished_at, 9, 2) AS INTEGER)
             FROM workout_sessions
             WHERE finished_at LIKE ?1 || '%'",
        )?;
        let rows = stmt.query_map(params![prefix], |row| row.get::<_, u32>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// Aggregates and returns the most frequently performed exercises based on the total count of completed sets.
    pub fn stats_top_exercises(&self, limit: usize) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT exercise_name, COUNT(*) AS cnt
             FROM session_sets
             WHERE completed = 1
             GROUP BY exercise_name
             ORDER BY cnt DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// Compiles a progression report comparing the current all-time best max-weight PR against the immediately preceding PR for each exercise.
    pub fn stats_pr_progression(&self, limit: usize) -> Result<Vec<(String, f32, f32)>> {
        let conn = self.conn.borrow();
        // Get exercises with max-weight PRs, ordered by current best desc
        let mut stmt = conn.prepare(
            "SELECT e.name,
                    (SELECT MAX(pr.value) FROM personal_records pr WHERE pr.exercise_id = e.id AND pr.record_type = 'Max Weight') AS best,
                    COALESCE(
                        (SELECT MAX(pr2.value) FROM personal_records pr2
                         WHERE pr2.exercise_id = e.id AND pr2.record_type = 'Max Weight'
                         AND pr2.id < (SELECT MAX(pr3.id) FROM personal_records pr3 WHERE pr3.exercise_id = e.id AND pr3.record_type = 'Max Weight')),
                        0.0
                    ) AS prev
             FROM exercises e
             WHERE (SELECT COUNT(*) FROM personal_records pr WHERE pr.exercise_id = e.id AND pr.record_type = 'Max Weight') > 0
             ORDER BY best DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f32>(1)?,
                row.get::<_, f32>(2)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// Groups workout sessions by their template name to calculate the total usage frequency of each generated routine.
    pub fn stats_plan_session_counts(&self) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT template_name, COUNT(*) AS cnt
             FROM workout_sessions
             GROUP BY template_name
             ORDER BY cnt DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }
}
