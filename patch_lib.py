import re

with open("src/lib.rs", "r", encoding="utf-8") as f:
    content = f.read()

helper_code = """
fn get_svg_strings() -> &'static std::collections::HashMap<i64, (String, String)> {
    static SVGS: std::sync::OnceLock<std::collections::HashMap<i64, (String, String)>> = std::sync::OnceLock::new();
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

fn get_cached_images(id: i64) -> Option<(slint::Image, slint::Image)> {
    thread_local! {
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
"""

content = content.replace("/// Rebuild the exercise list from the database.\n", helper_code)

old_map = """        .map(|exercise| ExerciseRow {
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
            has_images: false,
            image_relaxed: slint::Image::default(),
            image_tension: slint::Image::default(),
        })"""

new_map = """        .map(|exercise| {
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
        })"""

content = content.replace(old_map, new_map)

with open("src/lib.rs", "w", encoding="utf-8") as f:
    f.write(content)
