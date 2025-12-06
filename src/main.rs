use clap::{Arg, Command};
use pulldown_cmark::{Parser, Options, Event, Tag, HeadingLevel, LinkType};
use std::{
    fs,
    path::{Path, PathBuf},
    io::{self, Write},
    collections::HashSet,
};

/// Structure to hold arguments parsed by clap.
#[derive(Debug)]
struct Args {
    source: PathBuf,
    target: PathBuf,
    verbose: bool,
}

/// A global map of all files to easily check for links.
type SiteMap = HashSet<PathBuf>;

fn main() -> io::Result<()> {
    let args = parse_args();

    if args.verbose {
        println!("Verbose mode enabled.");
        println!("Source directory: {}", args.source.display());
        println!("Target directory: {}", args.target.display());
    }

    // --- Target Directory Handling ---
    if args.target.exists() && args.target.is_dir() {
        if args.verbose {
            println!("Cleaning target directory contents: {}", args.target.display());
        }
    } else {
        if args.verbose {
            println!("Creating target directory: {}", args.target.display());
        }
    }
    // Ensure the target directory exists.
    fs::create_dir_all(&args.target)?;

    // --- Build Site Map and Process Files ---
    let site_map = build_site_map(&args.source)?;
    if args.verbose {
        println!("Identified {} files for processing.", site_map.len());
    }
    
    // Process all files.
    process_directory(&args, &site_map, &args.source)?;
    
    // --- Index File Generation (FINAL DIAGNOSTIC VERSION) ---
    generate_all_index_files(&args, &site_map)?;

    println!("Done processing directories.");
    Ok(())
}

// 1. Argument Parsing using clap
fn parse_args() -> Args {
    let matches = Command::new("Directory Copier")
        .version("1.0")
        .author("Your Name <your-email@example.com>")
        .about("A program that copies directory structure with markdown to HTML conversion.")
        .arg(
            Arg::new("source")
                .short('s') 
                .long("source")
                .value_parser(clap::value_parser!(String))
                .value_name("SOURCE_DIR")
                .help("Specifies the source directory (defaults to current directory if not provided)"),
        )
        .arg(
            Arg::new("target")
                .short('t') 
                .long("target")
                .value_parser(clap::value_parser!(String))
                .required(true)
                .value_name("TARGET_DIR")
                .help("Specifies the target directory where files will be copied"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(clap::ArgAction::SetTrue)
                .help("Enables verbose output"),
        )
        .get_matches();

    // Get the source directory, defaulting to current_dir
    let source_dir_str = matches
        .get_one::<String>("source")
        .cloned()
        .unwrap_or_else(|| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| {
                    eprintln!("Error: Unable to determine the current directory.");
                    std::process::exit(1);
                })
        });

    let target_dir_str = matches.get_one::<String>("target").unwrap();

    Args {
        source: PathBuf::from(source_dir_str),
        target: PathBuf::from(target_dir_str),
        verbose: *matches.get_one::<bool>("verbose").unwrap_or(&false),
    }
}

// 2. Build Site Map
/// Traverses the source directory to collect all file paths relative to the source root.
fn build_site_map(source_dir: &Path) -> io::Result<SiteMap> {
    let mut site_map = HashSet::new();
    
    // Helper recursive function
    fn traverse(dir: &Path, source_root: &Path, map: &mut SiteMap) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                // Ignore hidden directories
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if name.starts_with('.') {
                        continue;
                    }
                }
                traverse(&path, source_root, map)?;
            } else if path.is_file() {
                if let Ok(rel_path) = path.strip_prefix(source_root) {
                    map.insert(rel_path.to_path_buf());
                }
            }
        }
        Ok(())
    }

    traverse(source_dir, source_dir, &mut site_map)?;
    Ok(site_map)
}

// 3. Process Directories and Files
fn process_directory(args: &Args, site_map: &SiteMap, current_dir_source: &Path) -> io::Result<()> {
    let current_dir_rel = current_dir_source.strip_prefix(&args.source).unwrap_or(Path::new(""));
    let current_dir_target = args.target.join(current_dir_rel);

    // 3a. Create target directory
    fs::create_dir_all(&current_dir_target)?;

    for entry in fs::read_dir(current_dir_source)? {
        let entry = entry?;
        let path_source = entry.path();

        if path_source.is_dir() {
            // Ignore hidden directories
            if let Some(name) = path_source.file_name().and_then(|s| s.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }
            process_directory(args, site_map, &path_source)?;
        } else if path_source.is_file() {
            let file_name = path_source.file_name().unwrap_or_default();
            let path_target = current_dir_target.join(file_name);
            
            let rel_path = path_source.strip_prefix(&args.source).unwrap_or(Path::new(""));
            
            if rel_path.extension().map_or(false, |ext| ext == "md") {
                // 3b. Markdown to HTML conversion
                markdown_to_html(args, site_map, &path_source, &path_target, rel_path)?;
            } else {
                // 3c. Copy other files
                fs::copy(&path_source, &path_target)?;
                if args.verbose {
                    println!("Copied: {}", rel_path.display());
                }
            }
        }
    }
    Ok(())
}

// 4. Markdown to HTML Logic
fn markdown_to_html(args: &Args, site_map: &SiteMap, path_source: &Path, path_target: &Path, path_rel: &Path) -> io::Result<()> {
    let markdown_input = fs::read_to_string(path_source)?;
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    
    let parser = Parser::new_ext(&markdown_input, options);
    
    let (html_output_content, title) = process_markdown_events(args, site_map, parser, path_rel);
    
    // Change target extension to .html
    let mut path_target_html = path_target.to_path_buf();
    path_target_html.set_extension("html");
    
    // Generate navigation tree HTML
    let nav_html = generate_navigation_html(args, site_map, path_rel);
    
    let final_html = format_html_page(&title, &nav_html, &html_output_content);
    
    fs::write(&path_target_html, final_html)?;

    if args.verbose {
        println!("Converted: {} -> {}", path_rel.display(), path_rel.with_extension("html").display());
    }

    Ok(())
}

/// Extracts title and performs link rewriting and validation during parsing.
fn process_markdown_events<'a>(
    _args: &Args, 
    site_map: &SiteMap,
    parser: Parser<'a, 'a>,
    path_rel: &Path,
) -> (String, String) {
    let mut title = String::new();
    let mut in_h1 = false;
    let mut events = Vec::new();
    let mut html_output = String::new(); 

    for event in parser {
        match event {
            // Title extraction logic
            Event::Start(Tag::Heading(HeadingLevel::H1, _, _)) if title.is_empty() => {
                in_h1 = true;
                events.push(Event::Start(Tag::Heading(HeadingLevel::H1, None, Vec::new())));
            }
            Event::Text(text) if in_h1 => {
                title.push_str(&text);
                events.push(Event::Text(text));
            }
            Event::End(Tag::Heading(HeadingLevel::H1, _, _)) if in_h1 => {
                in_h1 = false;
                events.push(Event::End(Tag::Heading(HeadingLevel::H1, None, Vec::new())));
            }
            // Link rewriting and validation
            Event::Start(Tag::Link(link_type, dest, title_attr)) => {
                if link_type == LinkType::Inline {
                    let mut dest_path = PathBuf::from(&*dest);
                    
                    if dest_path.extension().map_or(false, |ext| ext == "md") {
                        // Link Rewrite: .md -> .html
                        dest_path.set_extension("html");
                    }
                    
                    // Link Validation
                    let resolved_pathbuf = resolve_link_path(path_rel, &dest_path);
                    let resolved_link_str = resolved_pathbuf.to_string_lossy().into_owned();
                    
                    if resolved_link_str.starts_with('/') {
                         if let Ok(site_rel_path) = resolved_pathbuf.strip_prefix("/") {
                            // Check if the target is in the sitemap or is a generated index/styles.css
                            let is_site_file = site_map.contains(site_rel_path) || site_rel_path.ends_with("index.html") || site_rel_path.ends_with("styles.css");
                            if !is_site_file {
                                eprintln!("Warning: Broken internal link in {}: {} (Resolved to: {})", path_rel.display(), dest, resolved_pathbuf.display());
                            }
                        }
                    } 
                    
                    events.push(Event::Start(Tag::Link(link_type, resolved_link_str.into(), title_attr)));
                } else {
                    events.push(Event::Start(Tag::Link(link_type, dest, title_attr)));
                }
            }
            e => events.push(e),
        }
    }

    // Fallback title if no H1 found
    if title.is_empty() {
        title = path_rel.to_string_lossy().to_string();
        // Prepend a dummy H1 for the page content
        let fallback_h1 = format!("<h1 class=\"fallback-title\">{}</h1>", title);
        html_output = fallback_h1; 
    }

    let final_content = html_output + &events_to_html(events);
    
    (final_content, title)
}

/// Converts a vector of Pulldown events back to HTML string.
fn events_to_html(events: Vec<Event>) -> String {
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, events.into_iter());
    html_output
}

/// Resolves a potentially relative link path to a root-relative path for checking/output.
fn resolve_link_path(from_path_rel: &Path, link_target: &Path) -> PathBuf {
    if link_target.to_string_lossy().starts_with('/') {
        return link_target.to_path_buf();
    }
    
    // Get the directory of the file we are starting from
    let from_dir = from_path_rel.parent().unwrap_or(Path::new(""));
    
    let resolved_path = from_dir.join(link_target);
    
    // Normalize path to handle ".."
    let mut components = Vec::new();
    for component in resolved_path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::Normal(name) => {
                components.push(name);
            }
            _ => {}
        }
    }
    
    // Reconstruct the root-relative path (starting with /)
    let final_path = PathBuf::from("/").join(components.iter().collect::<PathBuf>());
    final_path
}

// Robust helper function for finding ALL directories (including directory-only folders like 2025)
fn collect_all_dirs_robust(source_dir: &Path) -> io::Result<HashSet<PathBuf>> {
    let mut dirs = HashSet::new();
    
    // Start traversal from the root directory
    let mut stack = vec![source_dir.to_path_buf()];

    while let Some(current_dir) = stack.pop() {
        // Calculate the relative path immediately
        let rel_path = current_dir.strip_prefix(source_dir).unwrap_or(Path::new(""));
        // This ensures the current directory's relative path is added.
        dirs.insert(rel_path.to_path_buf()); 
        
        for entry in fs::read_dir(&current_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // Ignore hidden directories
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


// 5. Index File Generation (FINAL DIAGNOSTIC VERSION)
fn generate_all_index_files(args: &Args, site_map: &SiteMap) -> io::Result<()> {
    
    // --- GUARANTEED COLLECTION OF ALL DIRECTORY PATHS ---
    let dirs_to_index = collect_all_dirs_robust(&args.source)?;
    
    let mut sorted_dirs: Vec<PathBuf> = dirs_to_index.into_iter().collect();
    sorted_dirs.sort();


    for rel_dir_path in sorted_dirs {
        let index_md_path = rel_dir_path.join("index.md");
        let path_target_dir = args.target.join(&rel_dir_path);
        let path_target = path_target_dir.join("index.html");

        let (title, content) = if site_map.contains(&index_md_path) {
            // Process index.md if it exists
            let path_source = args.source.join(&index_md_path);
            let markdown_input = fs::read_to_string(&path_source)?;
            let parser = Parser::new(&markdown_input);
            let (html_output, title) = process_markdown_events(args, site_map, parser, &index_md_path);
            (title, html_output)
        } else {
            // Index.md missing - Fallback logic (This should run for your 2025 folder)
            eprintln!("Warning: No index.md found in directory: {}", rel_dir_path.display());
            let title = if rel_dir_path.as_os_str().is_empty() {
                "Root Index".to_string()
            } else {
                rel_dir_path.to_string_lossy().to_string()
            };
            ("Index: ".to_string() + &title, String::new())
        };

        // Generate navigation and final HTML
        let nav_rel_path = if site_map.contains(&index_md_path) {
            index_md_path.clone()
        } else {
            rel_dir_path.join("index.md") // Placeholder path for navigation generation
        };
        
        let nav_html = generate_navigation_html(args, site_map, &nav_rel_path);
        let final_html = format_html_page(&title, &nav_html, &content);
        
        // --- CRITICAL STEP 1: Explicitly check for target directory creation ---
        match fs::create_dir_all(&path_target_dir) {
            Ok(_) => {
                 if args.verbose {
                    println!("Ensured target directory exists: {}", path_target_dir.display());
                }
            },
            Err(e) => {
                eprintln!("FATAL ERROR: Failed to create target directory {}: {}", path_target_dir.display(), e);
                return Err(e); // Propagate the error explicitly
            }
        }
        
        // --- CRITICAL STEP 2: Explicitly check for file write error ---
        match fs::write(&path_target, final_html) {
            Ok(_) => {
                if args.verbose {
                    println!("Successfully generated index.html at: {}", path_target.display());
                } else {
                    println!("Generated index.html for: {}", rel_dir_path.display());
                }
            }
            Err(e) => {
                eprintln!("FATAL ERROR: Failed to write index.html to {}: {}", path_target.display(), e);
                return Err(e); // Propagate the error explicitly
            }
        }
    }

    Ok(())
}

// 6. HTML Formatting
fn format_html_page(title: &str, nav_html: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <link rel="stylesheet" href="/styles.css">
</head>
<body>
    <header>
        {}
    </header>
    <main>
        {}
    </main>
</body>
</html>"#,
        title, nav_html, content
    )
}

// 7. Navigation Tree Generation
fn generate_navigation_html(args: &Args, site_map: &SiteMap, current_rel_path: &Path) -> String { 
    // The directory of the current file being viewed (e.g., /blog/2025)
    let current_dir_rel = current_rel_path.parent().unwrap_or(Path::new(""));
    
    // Components of the current directory path (e.g., ["blog", "2025"])
    let components: Vec<String> = current_dir_rel
        .iter()
        .filter_map(|s| s.to_str().map(|s| s.to_string()))
        .collect();
    let depth = components.len(); 

    let mut nav_html = String::from("<h2>Navigation</h2><ul>");
    
    // --- 1. Root Link ---
    nav_html.push_str(&format!(
        "<li><a href=\"/index.html\" title=\"Site Root\">/</a></li>"
    ));

    // --- 2. Path Breadcrumb (Lists all parents as links) ---
    let mut current_path_builder = PathBuf::new();
    
    // Loop through ALL parents *except* the current directory itself.
    for component in components.iter().take(depth.saturating_sub(1)) {
        current_path_builder.push(component);
        
        let path_link = format!("/{}/index.html", current_path_builder.to_string_lossy()); 
        
        nav_html.push_str(&format!(
            "<li><a href=\"{}\" title=\"Directory: {}\">{}</a></li>",
            path_link, current_path_builder.display(), component
        ));
    }

    // --- 3. Current Directory Contents (Siblings and Children) ---
    let current_dir_name = current_dir_rel.file_name().map_or("Root", |s| s.to_str().unwrap_or(""));
    
    // Display current directory name (NON-LINK) as the list header
    nav_html.push_str(&format!(
        "<li class=\"current-dir-list\">{}: <ul>",
        current_dir_name
    ));

    let mut files = Vec::new();
    let mut subdirs: HashSet<PathBuf> = HashSet::new();

    // Pass 1: Find direct children based on files in the site map
    for rel_path in site_map.iter() {
        if let Some(parent) = rel_path.parent() {
            // Case 1: Sibling file (file directly in current directory)
            if parent == current_dir_rel {
                if !rel_path.ends_with("styles.css") {
                    files.push(rel_path.to_path_buf());
                }
            } 
            // Case 2: Child directory: Use strip_prefix to find the direct child's name
            else if rel_path.starts_with(current_dir_rel) {
                if let Ok(path_suffix) = rel_path.strip_prefix(current_dir_rel) {
                    if let Some(first_component) = path_suffix.components().next() {
                        if let Some(dir_name) = first_component.as_os_str().to_str() {
                            let child_dir_path = current_dir_rel.join(dir_name);
                            subdirs.insert(child_dir_path);
                        }
                    }
                }
            } 
        }
    }

    // Pass 2: Read source directory directly to find empty/dir-only subdirectories
    if let Ok(dir_entries) = fs::read_dir(Path::new(&args.source).join(current_dir_rel)) {
        for entry in dir_entries {
            if let Ok(entry) = entry {
                if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                    if let Some(name) = entry.file_name().to_str() {
                        if !name.starts_with('.') {
                            let child_dir_path = current_dir_rel.join(name);
                            subdirs.insert(child_dir_path);
                        }
                    }
                }
            }
        }
    }
    
    // Add child directories first
    let mut sorted_subdirs: Vec<PathBuf> = subdirs.into_iter().collect();
    sorted_subdirs.sort();

    for dir_path in sorted_subdirs {
        let dir_name = dir_path.file_name().unwrap().to_string_lossy();
        let link_path = format!("/{}/index.html", dir_path.to_string_lossy());
        nav_html.push_str(&format!(
            "<li><a href=\"{}\" title=\"Directory Index: {}\">{} (Dir)</a></li>",
            link_path, dir_path.display(), dir_name
        ));
    }

    // Add sibling files
    files.sort();
    
    for rel_path in files {
        let is_current = rel_path.with_extension("html") == current_rel_path.with_extension("html");
        
        let (link_path, file_name) = if rel_path.extension().map_or(false, |ext| ext == "md") {
            (format!("/{}", rel_path.with_extension("html").to_string_lossy()), rel_path.file_stem().unwrap().to_string_lossy().to_string() + ".html")
        } else {
            (format!("/{}", rel_path.to_string_lossy()), rel_path.file_name().unwrap().to_string_lossy().to_string())
        };
        
        let title_attr = rel_path.to_string_lossy(); 
        
        if is_current {
             nav_html.push_str(&format!(
                "<li class=\"current-file\" title=\"{}\">{}</li>",
                title_attr, file_name
            ));
        } else {
            nav_html.push_str(&format!(
                "<li><a class=\"nav-link\" href=\"{}\" title=\"{}\">{}</a></li>",
                link_path, title_attr, file_name
            ));
        }
    }

    nav_html.push_str("</ul></li>");
    nav_html.push_str("</ul>");
    nav_html
}