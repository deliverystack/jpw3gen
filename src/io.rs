use crate::config::{Args, COLOR_CYAN, COLOR_RED, COLOR_RESET, COLOR_YELLOW};
use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

pub fn print_error(message: &str) {
    eprintln!("{}ERROR{}: {}", COLOR_RED, COLOR_RESET, message);
}

pub fn print_warning(message: &str) {
    eprintln!("{}WARNING{}: {}", COLOR_YELLOW, COLOR_RESET, message);
}

pub fn print_info(message: &str) {
    eprintln!("{}INFO{}: {}", COLOR_CYAN, COLOR_RESET, message);
}

pub fn read_template(source_dir: &Path, args: &Args) -> io::Result<String> {
    let template_path = source_dir.join("template.html");

    if args.verbose {
        print_info(&format!(
            "Attempting to read HTML template from: {}",
            template_path.display()
        ));
    }

    match fs::read_to_string(&template_path) {
        Ok(template) => {
            if args.verbose {
                print_info("Successfully read custom template.html.");
            }
            Ok(template)
        }
        Err(e) => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Required file template.html not found at {}: {}",
                template_path.display(),
                e
            ),
        )),
    }
}

pub fn collect_all_dirs_robust(source_dir: &Path) -> io::Result<HashSet<PathBuf>> {
    let mut dirs = HashSet::new();
    let mut stack = vec![source_dir.to_path_buf()];

    while let Some(current_dir) = stack.pop() {
        let rel_path = current_dir
            .strip_prefix(source_dir)
            .unwrap_or(Path::new(""));
        dirs.insert(rel_path.to_path_buf());

        for entry in fs::read_dir(&current_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if name.starts_with('.') {
                        continue;
                    }
                }
                stack.push(path);
            }
        }
    }

    Ok(dirs)
}
