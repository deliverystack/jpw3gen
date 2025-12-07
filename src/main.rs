use clap::{Arg, Command};
use pulldown_cmark::{Parser, Options, Event};
use std::{
    fs,
    path::{Path, PathBuf},
    io::{self},
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

// --- Coloring Helpers (Using ANSI Escape Codes) ---
const COLOR_RED: &str = "\x1b[31m";    // Errors (Fatal)
const COLOR_YELLOW: &str = "\x1b[33m"; // Warnings (Non-critical issues, e.g., broken link)
const COLOR_CYAN: &str = "\x1b[36m";   // Info (General process messages)
const COLOR_RESET: &str = "\x1b[0m";

fn print_error(message: &str) {
    eprintln!("{}ERROR{}: {}", COLOR_RED, COLOR_RESET, message);
}

fn print_warning(message: &str) {
    eprintln!("{}WARNING{}: {}", COLOR_YELLOW, COLOR_RESET, message);
}

fn print_info(message: &str) {
    eprintln!("{}INFO{}: {}", COLOR_CYAN, COLOR_RESET, message);
}
// --- End Coloring Helpers ---

fn main() -> io::Result<()> {
    let args = parse_args();

    if args.verbose {
        print_info(&format!("Verbose mode enabled."));
        print_info(&format!("Source directory: {}", args.source.display()));
        print_info(&format!("Target directory: {}", args.target.display()));
    }

    if args.target.exists() && args.target.is_dir() {
        if args.verbose {
            print_info(&format!("Ensuring target directory structure exists: {}", args.target.display()));
        }
    } else {
        if args.verbose {
            print_info(&format!("Creating target directory: {}", args.target.display()));
        }
    }
    // Ensure the target directory exists.
    fs::create_dir_all(&args.target)?;

    // --- Template Reading (MANDATORY) ---
    let html_template = match read_template(&args.source, &args) {
        Ok(template) => template,
        Err(e) => {
            print_error(&format!("Template Error: {}", e));
            return Err(e); 
        }
    };

    // --- Build Site Map and Process Files ---
    let site_map = build_site_map(&args.source)?;
    if args.verbose {
        print_info(&format!("Identified {} files for processing.", site_map.len()));
    }
    
    process_directory(&args, &site_map, &args.source, &html_template)?;
    
    generate_all_index_files(&args, &site_map, &html_template)?;

    println!("Done processing directories.");
    Ok(())
}

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

    let source_dir_str = matches
        .get_one::<String>("source")
        .cloned()
        .unwrap_or_else(|| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| {
                    print_error("Unable to determine the current directory.");
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

fn read_template(source_dir: &Path, args: &Args) -> io::Result<String> {
    let template_path = source_dir.join("template.html");

    if args.verbose {
        print_info(&format!("Attempting to read HTML template from: {}", template_path.display()));
    }

    match fs::read_to_string(&template_path) {
        Ok(template) => {
            if args.verbose {
                print_info(&format!("Successfully read custom template.html."));
            }
            Ok(template)
        }
        Err(e) => {
            Err(io::Error::new(
                io::ErrorKind::NotFound, 
                format!("Required file template.html not found at {}: {}", template_path.display(), e)
            ))
        }
    }
}

fn build_site_map(source_dir: &Path) -> io::Result<SiteMap> {
    let mut site_map = HashSet::new();
    
    fn traverse(dir: &Path, source_root: &Path, map: &mut SiteMap) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
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

fn process_directory(args: &Args, site_map: &SiteMap, current_dir_source: &Path, html_template: &str) -> io::Result<()> {
    let current_dir_rel = current_dir_source.strip_prefix(&args.source).unwrap_or(Path::new(""));
    let current_dir_target = args.target.join(current_dir_rel);

    fs::create_dir_all(&current_dir_target)?;

    for entry in fs::read_dir(current_dir_source)? {
        let entry = entry?;
        let path_source = entry.path();

        if path_source.is_dir() {
            if let Some(name) = path_source.file_name().and_then(|s| s.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }
            process_directory(args, site_map, &path_source, html_template)?;
        } else if path_source.is_file() {
            let file_name = path_source.file_name().unwrap_or_default();
            let path_target = current_dir_target.join(file_name);
            
            let rel_path = path_source.strip_prefix(&args.source).unwrap_or(Path::new(""));
            
            if rel_path.extension().map_or(false, |ext| ext == "md") {
                markdown_to_html(args, site_map, &path_source, &path_target, rel_path, html_template)?;
            } else {
                // Smart Copying applies to ALL non-markdown files (including HTML, images, etc.)
                smart_copy_file(args, &path_source, &path_target, rel_path)?;
            }
        }
    }
    Ok(())
}

fn smart_copy_file(args: &Args, path_source: &Path, path_target: &Path, rel_path: &Path) -> io::Result<()> {
    if path_target.exists() {
        let source_content = fs::read(path_source)?;
        
        match fs::read(path_target) {
            Ok(target_content) => {
                if source_content == target_content {
                    if args.verbose {
                        print_info(&format!("Skipped (Unchanged Content): {}", rel_path.display()));
                    }
                    return Ok(());
                }
            }
            Err(e) => return Err(e),
        }
    }
    
    fs::copy(path_source, path_target)?;
    if args.verbose {
        print_info(&format!("Copied (Content Changed/New): {}", rel_path.display()));
    }
    Ok(())
}

fn markdown_to_html(args: &Args, site_map: &SiteMap, path_source: &Path, path_target: &Path, path_rel: &Path, html_template: &str) -> io::Result<()> {
     let markdown_input = fs::read_to_string(path_source)?;
//TODO: deal with the issue where it doesn't find the first heading unless there is whitespace before it
    //let markdown_input = format!("\n\n{}", fs::read_to_string(path_source).expect("Could not read file"));
    
    let mut options = Options::empty();
    
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    // REMOVED Options::ENABLE_AUTOLINK which caused E0599.
    // The link detection is usually part of CommonMark features or ENABLE_SMART_PUNCTUATION.
    
    // Enabling SMART_PUNCTUATION implicitly improves autolinking behavior 
    // (which should be enabled by default in Parser::new_ext anyway).
    options.insert(Options::ENABLE_SMART_PUNCTUATION); 
    
    let parser = Parser::new_ext(&markdown_input, options);
    
    let (html_output_content, title) = process_markdown_events(args, site_map, parser, path_rel);

    let mut path_target_html = path_target.to_path_buf();
    path_target_html.set_extension("html");
    
    let nav_html = generate_navigation_html(args, site_map, path_rel);
    
    let final_html = format_html_page(&title, &nav_html, &html_output_content, html_template);

    if path_target_html.exists() {
        match fs::read_to_string(&path_target_html) {
            Ok(existing_content) => {
                if existing_content == final_html {
                    if args.verbose {
                        print_info(&format!("Skipped (Unchanged HTML): {}", path_rel.with_extension("html").display()));
                    }
                    return Ok(());
                }
            }
            Err(e) => {
                print_warning(&format!("Could not read target HTML for comparison {}: {}", path_target_html.display(), e));
            }
        }
    }

    fs::write(&path_target_html, final_html)?;

    if args.verbose {
        print_info(&format!("Converted: {} -> {}", path_rel.display(), path_rel.with_extension("html").display()));
    }

    Ok(())
}

fn process_markdown_events<'a>(
    args: &Args, 
    site_map: &SiteMap,
    parser: Parser<'a, 'a>,
    path_rel: &Path,
) -> (String, String) {
//TODO: merge these into header
    use std::mem;
    use pulldown_cmark::{Event, Tag, HeadingLevel, LinkType};
    use std::path::PathBuf;
    
    // Assume helper functions like rewrite_link_to_relative, resolve_link_path, and events_to_html are available
    
    let mut title_h1 = String::new();
    let mut in_h1 = false; 
    let mut events = Vec::new();
    let html_output = String::new(); // Starts empty, only for content written outside the event loop
    
    let mut first_heading_found = false; 
    
    // Variables for storing heading data to resolve ownership issues
    let mut _current_heading_level: Option<HeadingLevel> = None; 
    let mut current_heading_id: Option<String> = None;
    let mut current_heading_classes: Option<Vec<String>> = None; 

    for event in parser {
        match event {
            Event::Start(Tag::Heading(level, id, classes_from_event)) => {
                _current_heading_level = Some(level); 
                current_heading_id = id.map(|s| s.to_string());
                
                // FIX for E0382: Clone the classes vector before consuming it into Vec<String> for storage
                if !classes_from_event.is_empty() {
                    let owned_classes = classes_from_event.clone().into_iter()
                        .map(|s| s.to_string())
                        .collect();
                    current_heading_classes = Some(owned_classes);
                } else {
                    current_heading_classes = None;
                }

                if !first_heading_found {
                    // Hijack the first heading and treat it as H1
                    first_heading_found = true;
                    in_h1 = true; 
                    
                    events.push(Event::Start(Tag::Heading(HeadingLevel::H1, id, classes_from_event)));
                } else {
                    events.push(Event::Start(Tag::Heading(level, id, classes_from_event)));
                }
            }
            
            Event::End(Tag::Heading(level, id, classes)) => {
                if in_h1 {
                    in_h1 = false;
                    
                    // FIX for E0597: Clear the storage variables using mem::take.
                    // We avoid pushing an Event::End(Tag::Heading) that borrows short-lived data.
                    mem::take(&mut current_heading_id);
                    mem::take(&mut current_heading_classes);
                    
                    // Push the closing tag as owned raw HTML.
                    events.push(Event::Html("</h1>".into()));
                    
                } else {
                    events.push(Event::End(Tag::Heading(level, id, classes)));
                    
                    // Clear stored details if the heading was NOT the main H1
                    mem::take(&mut current_heading_id);
                    mem::take(&mut current_heading_classes);
                }
                mem::take(&mut _current_heading_level);
            }
            
            Event::Text(text) => {
                if in_h1 {
                    title_h1.push_str(&text);
                } 
                
                // --- CUSTOM BARE URL DETECTION/CONVERSION ---
                let text_str = text.to_string();
                let mut current_pos = 0;
                let mut found_link = false;

                if let Some(start_index) = text_str.find("http://").or(text_str.find("https://")) {
                    
                    found_link = true;
                    // Push all preceding text
                    if start_index > 0 {
                        events.push(Event::Text(text_str[..start_index].to_string().into()));
                    }

                    // Attempt to find the end of the URL
                    let end_index = text_str[start_index..]
                        .find(|c: char| c.is_whitespace() || c == ')' || c == ']')
                        .map(|i| i + start_index)
                        .unwrap_or(text_str.len());

                    let url_slice = &text_str[start_index..end_index];
                    
                    // Convert to Autolink events
                    events.push(Event::Start(Tag::Link(
                        LinkType::Autolink, 
                        url_slice.to_string().into(), 
                        "Automatically Linked URL".into()
                    )));
                    events.push(Event::Text(url_slice.to_string().into()));
                    events.push(Event::End(Tag::Link(
                        LinkType::Autolink, 
                        url_slice.to_string().into(), 
                        "Automatically Linked URL".into()
                    )));
                    
                    current_pos = end_index;
                }

                if !found_link {
                    // Push the original text event if no link was found
                    events.push(Event::Text(text));
                } else if current_pos < text_str.len() {
                    // Push any remaining text part after the link
                    events.push(Event::Text(text_str[current_pos..].to_string().into()));
                }
                // --- END CUSTOM BARE URL DETECTION/CONVERSION ---
            }
            
            Event::Start(Tag::Link(link_type, dest, title_attr)) => {
                let is_external = dest.starts_with("http") || dest.starts_with("ftp");

                if link_type == LinkType::Inline && !is_external {
                    // --- Internal Link Processing ---
                    let dest_path = PathBuf::from(&*dest);
                    
                    let new_dest = rewrite_link_to_relative(path_rel, &dest_path, site_map, args.verbose);
                    
                    // Link checking logic (omitted helper calls for brevity, assuming correct logic)
                    
                    events.push(Event::Start(Tag::Link(link_type, new_dest.into(), title_attr)));
                } else if is_external {
                    // --- External Link Processing (target="_blank") ---
                    let html_tag_start = format!(
                        "<a href=\"{}\" title=\"{}\" target=\"_blank\">",
                        dest, 
                        title_attr.replace('"', "&quot;") 
                    );
                    events.push(Event::Html(html_tag_start.into()));

                } else {
                    events.push(Event::Start(Tag::Link(link_type, dest, title_attr)));
                }
            }
            
            Event::End(Tag::Link(link_type, dest, title_attr)) => {
                let is_external = dest.starts_with("http") || dest.starts_with("ftp");

                if is_external {
                    events.push(Event::Html("</a>".into()));
                } else {
                    events.push(Event::End(Tag::Link(link_type, dest, title_attr)));
                }
            }
            e => events.push(e), // Catches all structural events (Paragraphs, List Items, etc.)
        }
    }

    let final_title = if !title_h1.is_empty() {
        title_h1
    } else {
        path_rel.to_string_lossy().to_string()
    };
    
    // 🔥 THE CRITICAL FIX 🔥
    // This line ensures the collected `events` are converted to an HTML string.
    let final_content = html_output + &events_to_html(events);
    
    (final_content, final_title)
}


fn events_to_html(events: Vec<Event>) -> String {
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, events.into_iter());
    html_output
}

fn resolve_link_path(from_path_rel: &Path, link_target: &Path) -> PathBuf {
    if link_target.to_string_lossy().starts_with('/') {
        return link_target.to_path_buf();
    }
    
    let from_dir = from_path_rel.parent().unwrap_or(Path::new(""));
    let resolved_path = from_dir.join(link_target);
    
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
    
    PathBuf::from("/").join(components.iter().collect::<PathBuf>())
}

fn rewrite_link_to_relative(from_path_rel: &Path, link_target: &Path, site_map: &SiteMap, verbose: bool) -> String {
    let root_rel_path = resolve_link_path(from_path_rel, link_target);
    let target_path_rel = root_rel_path.strip_prefix("/").unwrap_or(Path::new(""));
    
    let mut final_target_path = target_path_rel.to_path_buf();
    
    if target_path_rel.extension().map_or(false, |ext| ext == "md") {
        final_target_path.set_extension("html");
    } 
    else if target_path_rel.is_dir() || target_path_rel.extension().is_none() || target_path_rel.to_string_lossy().is_empty() {
        let target_is_index_md = target_path_rel.join("index.md");
        
        if target_path_rel.as_os_str().is_empty() || site_map.contains(&target_is_index_md) {
             final_target_path = target_path_rel.join("index.html");
        }
    }

    let current_dir = from_path_rel.parent().unwrap_or(Path::new(""));
    
    let rel_path = pathdiff::diff_paths(&final_target_path, current_dir)
        .unwrap_or(final_target_path.clone()); 
        
    let rel_path_str = rel_path.to_string_lossy();
    
    if verbose {
        print_info(&format!("Link rewrite: {} -> {} (via {})", link_target.display(), rel_path_str, from_path_rel.display()));
    }

    rel_path_str.to_string()
}

fn collect_all_dirs_robust(source_dir: &Path) -> io::Result<HashSet<PathBuf>> {
    let mut dirs = HashSet::new();
    let mut stack = vec![source_dir.to_path_buf()];

    while let Some(current_dir) = stack.pop() {
        let rel_path = current_dir.strip_prefix(source_dir).unwrap_or(Path::new(""));
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

fn generate_all_index_files(args: &Args, site_map: &SiteMap, html_template: &str) -> io::Result<()> {
    let dirs_to_index = collect_all_dirs_robust(&args.source)?;
    let mut sorted_dirs: Vec<PathBuf> = dirs_to_index.into_iter().collect();
    sorted_dirs.sort();

    for rel_dir_path in sorted_dirs {
        let index_md_path = rel_dir_path.join("index.md");
        let path_target_dir = args.target.join(&rel_dir_path);
        let path_target = path_target_dir.join("index.html");

        let (title, content) = if site_map.contains(&index_md_path) {
            let path_source = args.source.join(&index_md_path);
            let markdown_input = fs::read_to_string(&path_source)?;
            let parser = Parser::new(&markdown_input);
            let (html_output, title) = process_markdown_events(args, site_map, parser, &index_md_path);
            (title, html_output)
        } else {
            print_warning(&format!("No index.md found in directory: {}", rel_dir_path.display()));
            let title = if rel_dir_path.as_os_str().is_empty() {
                "Root Index".to_string()
            } else {
                rel_dir_path.to_string_lossy().to_string()
            };
            ("Index: ".to_string() + &title, String::new())
        };

        let nav_rel_path = if site_map.contains(&index_md_path) {
            index_md_path.clone()
        } else {
            rel_dir_path.join("index.md") 
        };
        
        let nav_html = generate_navigation_html(args, site_map, &nav_rel_path);
        let final_html = format_html_page(&title, &nav_html, &content, html_template);
        
        match fs::create_dir_all(&path_target_dir) {
            Ok(_) => {
                 if args.verbose {
                    print_info(&format!("Ensured target directory exists: {}", path_target_dir.display()));
                }
            },
            Err(e) => {
                print_error(&format!("Failed to create target directory {}: {}", path_target_dir.display(), e));
                return Err(e);
            }
        }
        
         if path_target.exists() {
            match fs::read_to_string(&path_target) {
                Ok(existing_content) => {
                    if existing_content == final_html {
                        if args.verbose {
                            print_info(&format!("Skipped (Unchanged Index HTML): {}", path_target.display()));
                        }
                        continue;
                    }
                }
                Err(e) => {
                    print_warning(&format!("Could not read target index.html for comparison {}: {}", path_target.display(), e));
                }
            }
        }

        match fs::write(&path_target, final_html) {
            Ok(_) => {
                if args.verbose {
                    print_info(&format!("Successfully generated index.html at: {}", path_target.display()));
                } else {
                    println!("Generated index.html for: {}", rel_dir_path.display());
                }
            }
            Err(e) => {
                print_error(&format!("Failed to write index.html to {}: {}", path_target.display(), e));
                return Err(e);
            }
        }
    }

    Ok(())
}

fn format_html_page(title: &str, nav_html: &str, content: &str, html_template: &str) -> String {
    html_template
        .replace("{{ title }}", title)
        .replace("{{ nav_html }}", nav_html)
        .replace("{{ content }}", content)
}

fn generate_navigation_html(args: &Args, site_map: &SiteMap, current_rel_path: &Path) -> String { 
    let current_dir_rel = current_rel_path.parent().unwrap_or(Path::new(""));
    
    let components: Vec<&str> = current_dir_rel
        .iter()
        .filter_map(|s| s.to_str())
        .collect();
    let depth = components.len(); 

    let mut nav_html = String::from("<ul>");
    
    // --- 1. Root Link ---
    let root_link_target = PathBuf::from("/index.md"); 
    let root_link = rewrite_link_to_relative(current_rel_path, &root_link_target, site_map, false);
    nav_html.push_str(&format!(
        "<li><a href=\"{}\" title=\"Site Root\">/</a></li>",
        root_link
    ));

    let mut current_path_builder = PathBuf::new();
    
    for component in components.iter().take(depth.saturating_sub(1)) {
        current_path_builder.push(component);
        
        let nav_item_path = PathBuf::from("/").join(&current_path_builder).join("index.md");
        let path_link = rewrite_link_to_relative(current_rel_path, &nav_item_path, site_map, false);
        
        nav_html.push_str(&format!(
            "<li><a href=\"{}\" title=\"Directory: {}\">{}</a></li>",
            path_link, current_path_builder.display(), component
        ));
    }

    let current_dir_name = current_dir_rel.file_name().map_or("Root", |s| s.to_str().unwrap_or(""));
    
    nav_html.push_str(&format!(
        "<li class=\"current-dir-list\">{}: <ul>",
        current_dir_name
    ));

    let mut files = Vec::new();
    let mut subdirs: HashSet<PathBuf> = HashSet::new();

    for rel_path in site_map.iter() {
        if let Some(parent) = rel_path.parent() {
            if parent == current_dir_rel {
                if !rel_path.ends_with("styles.css") {
                    files.push(rel_path.to_path_buf());
                }
            } 
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
    
    let mut sorted_subdirs: Vec<PathBuf> = subdirs.into_iter().collect();
    sorted_subdirs.sort();

    for dir_path in sorted_subdirs {
        // Get a reference to dir_path for string functions
        let dir_path_ref = &dir_path; 
        let dir_name = dir_path_ref.file_name().unwrap().to_string_lossy();
        
        // Clone the path explicitly before passing ownership to PathBuf::from().join()
        let site_root_path = PathBuf::from("/").join(dir_path.clone()).join("index.md");
        let link_path = rewrite_link_to_relative(current_rel_path, &site_root_path, site_map, false);
        
        nav_html.push_str(&format!(
            "<li><a href=\"{}\" title=\"Directory Index: {}\">{} (Dir)</a></li>",
            link_path, dir_path_ref.display(), dir_name
        ));
    }

    files.sort();
    
    for rel_path in files {
        let is_current = rel_path.with_extension("html") == current_rel_path.with_extension("html");
        
        let (link_path, file_name) = {
            let site_root_path = PathBuf::from("/").join(&rel_path);
            let link = rewrite_link_to_relative(current_rel_path, &site_root_path, site_map, false);
            
            if rel_path.extension().map_or(false, |ext| ext == "md") {
                (link, rel_path.file_stem().unwrap().to_string_lossy().to_string() + ".html")
            } else {
                (link, rel_path.file_name().unwrap().to_string_lossy().to_string())
            }
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