import glob
import json
import os

# Mappings to match the FastGTrack app categories
MUSCLE_MAP = {
    "abdominals": "Core",
    "pectoralis major": "Chest",
    "pectoralis minor": "Chest",
    "latissimus dorsi": "Upper Back",
    "trapezius": "Upper Back",
    "rhomboids": "Upper Back",
    "teres major": "Upper Back",
    "back": "Upper Back",
    "erector spinae": "Lower Back",
    "lower back": "Lower Back",
    "deltoid": "Shoulders",
    "deltoideus (clavicula)": "Shoulders",
    "should": "Shoulders",
    "biceps brachii": "Arms",
    "bicpes": "Arms",
    "triceps brachii": "Arms",
    "forearm": "Arms",
    "forerm": "Arms",
    "brachialis": "Arms",
    "rectus abdominis": "Core",
    "obliques": "Core",
    "transversus abdominis": "Core",
    "core": "Core",
    "quadriceps": "Legs",
    "hamstrings": "Legs",
    "ischiocrural muscles": "Legs",
    "calves": "Legs",
    "gastrocnemius": "Legs",
    "soleus": "Legs",
    "adductors": "Legs",
    "abductors": "Legs",
    "hip abductors": "Legs",
    "gluteus maximus": "Glutes",
    "glutaeus maximus": "Glutes",
    "gluteus medius": "Glutes",
    "gluteus minimus": "Glutes",
    "cardiovascular system": "Cardio",
}

EQUIPMENT_MAP = {
    "barbell": "Barbell",
    "ez-curl bar": "Barbell",
    "dumbbell": "Dumbbell",
    "machine": "Machine",
    "smith machine": "Machine",
    "cable": "Cable",
    "body": "Bodyweight",
    "bodyweight": "Bodyweight",
    "band": "Resistance Band",
    "exercise band": "Resistance Band",
    "none": "None",
}


def map_muscle(raw_muscle):
    if not raw_muscle:
        return "All Muscles"
    m = raw_muscle.lower()
    return MUSCLE_MAP.get(m, m.title())


def map_equipment(raw_equip):
    if not raw_equip:
        return "None"
    e = raw_equip.lower()
    return EQUIPMENT_MAP.get(e, e.title())


def clean_and_prepare():
    base_dir = os.path.dirname(os.path.abspath(__file__))
    data_dir = os.path.join(base_dir, "data")

    if not os.path.exists(data_dir):
        print(f"Error: {data_dir} not found.")
        return

    optimized_exercises = []

    # Iterate through all subdirectories in the data folder
    for folder_name in os.listdir(data_dir):
        folder_path = os.path.join(data_dir, folder_name)
        if not os.path.isdir(folder_path):
            continue

        # Clean up files: keep only .json and .svg
        for filename in os.listdir(folder_path):
            file_path = os.path.join(folder_path, filename)
            if os.path.isfile(file_path):
                ext = os.path.splitext(filename)[1].lower()
                if ext not in [".json", ".svg"]:
                    os.remove(file_path)
                    print(f"Deleted {file_path}")

        # Process the JSON file
        json_files = glob.glob(os.path.join(folder_path, "*.json"))
        if not json_files:
            print(f"No JSON found in {folder_name}")
            continue

        # Find SVGs to link them
        svg_files = sorted(glob.glob(os.path.join(folder_path, "*.svg")))

        svg_data_list = []
        for p in svg_files:
            try:
                with open(p, "r", encoding="utf-8") as f:
                    svg_data_list.append(f.read())
            except Exception as e:
                print(f"Failed to read SVG {p}: {e}")

        json_path = json_files[0]
        try:
            with open(json_path, "r", encoding="utf-8") as f:
                data = json.load(f)

            # Extract and transform fields for FastGTrack
            # Parse ID to int to match ExerciseRow struct
            ex_id = int(data.get("id", 0))
            title = data.get("title", "Unknown Exercise")
            primer = data.get("primer", "")

            # Map primary muscle
            primary_list = data.get("primary", [])
            primary_muscle = primary_list[0] if primary_list else ""
            mapped_muscle = map_muscle(primary_muscle)

            # Map equipment
            equip_list = data.get("equipment", [])
            equipment = equip_list[0] if equip_list else ""
            mapped_equip = map_equipment(equipment)

            # Form Meta string (e.g., "Chest • Barbell • Isolation")
            ex_type = data.get("type", "").title()
            meta = (
                f"{mapped_muscle} • {mapped_equip} • {ex_type}"
                if ex_type
                else f"{mapped_muscle} • {mapped_equip}"
            )

            # Add to optimized list
            optimized_exercises.append(
                {
                    "id": ex_id,
                    "name": title,
                    "meta": meta,
                    "description": primer,
                    "source": "system",  # Indicates built-in exercise
                    # SVG Paths for the UI to display animations/images
                    "svg_images": svg_data_list,
                    # We can also keep raw mapped values for the backend to use during filtering/sorting
                    "_filter_muscle": mapped_muscle,
                    "_filter_equipment": mapped_equip,
                }
            )

        except Exception as e:
            print(f"Failed to process {json_path}: {e}")

    # Save the consolidated JSON file
    # This file will be loaded by Rust on startup for instant in-memory searching/filtering
    output_file = os.path.join(base_dir, "optimized_exercises.json")

    # Sort by ID to keep it consistent
    optimized_exercises.sort(key=lambda x: x["id"])

    with open(output_file, "w", encoding="utf-8") as f:
        json.dump(optimized_exercises, f, indent=4, ensure_ascii=False)

    print(
        f"\nSuccessfully cleaned directory and processed {len(optimized_exercises)} exercises."
    )
    print(f"Optimized database saved to: {output_file}")


if __name__ == "__main__":
    clean_and_prepare()
