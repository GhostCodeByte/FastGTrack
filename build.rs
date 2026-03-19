use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let content = std::fs::read_to_string("ui/app.slint").unwrap_or_default();
    let open = content.chars().filter(|c| *c == '{').count();
    let close = content.chars().filter(|c| *c == '}').count();

    if open != close && !content.is_empty() {
        panic!(
            "Brace mismatch in ui/app.slint: {} open, {} close",
            open, close
        );
    }

    let _ = fs::create_dir_all("icons/planIcon");

    let mut icon_files = Vec::new();

    fn scan_dir(dir: &Path, files: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if ext.eq_ignore_ascii_case("png") || ext.eq_ignore_ascii_case("svg") {
                            files.push(path);
                        }
                    }
                }
            }
        }
    }

    scan_dir(Path::new("icons"), &mut icon_files);
    scan_dir(Path::new("icons/planIcon"), &mut icon_files);

    let mut slint_code = String::new();
    slint_code.push_str("export global IconData {\n");
    slint_code.push_str("    public pure function next-icon(current: string) -> string {\n");

    let mut icon_names = Vec::new();
    let mut paths_for_slint = Vec::new();

    for path in &icon_files {
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            let name = stem.to_string();
            let mut fpath = path.to_string_lossy().replace("\\", "/");
            if !fpath.starts_with("../") {
                fpath = format!("../{}", fpath);
            }
            icon_names.push(name);
            paths_for_slint.push(fpath);
        }
    }

    let mut unique_names = Vec::new();
    let mut unique_paths = Vec::new();
    for (name, path) in icon_names.into_iter().zip(paths_for_slint) {
        if !unique_names.contains(&name) {
            unique_names.push(name);
            unique_paths.push(path);
        }
    }

    for i in 0..unique_names.len() {
        let current = &unique_names[i];
        let next = &unique_names[(i + 1) % unique_names.len()];
        slint_code.push_str(&format!(
            "        if (current == \"{}\") {{ return \"{}\"; }}\n",
            current, next
        ));
    }
    if !unique_names.is_empty() {
        slint_code.push_str(&format!("        return \"{}\";\n", unique_names[0]));
    } else {
        slint_code.push_str("        return \"\";\n");
    }

    slint_code.push_str("    }\n}\n\n");

    slint_code.push_str("export component DynamicIcon inherits Image {\n");
    slint_code.push_str("    in property <string> icon-name;\n");
    slint_code.push_str("    source: ");

    for (i, p) in unique_paths.iter().enumerate() {
        let name = &unique_names[i];
        slint_code.push_str(&format!(
            "icon-name == \"{}\" ? @image-url(\"{}\") : ",
            name, p
        ));
    }

    if !unique_paths.is_empty() {
        slint_code.push_str(&format!("@image-url(\"{}\");\n", unique_paths[0]));
    } else {
        slint_code.push_str("@image-url(\"../icons/chest.png\");\n");
    }
    slint_code.push_str("}\n");

    fs::write("ui/generated_icons.slint", slint_code)
        .expect("Failed to write generated_icons.slint");

    slint_build::compile("ui/app.slint").expect("failed to compile Slint UI");
    println!("cargo:rerun-if-changed=icons");
    println!("cargo:rerun-if-changed=icons/planIcon");
}
