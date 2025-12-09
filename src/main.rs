use clap::{Arg, Command};
use pulldown_cmark::{Parser, Options, Event};
use std::{
    fs,
    path::{Path, PathBuf},
    io::{self},
    collections::HashSet,
};


use std::collections::BTreeMap; 

#[derive(Debug, Clone)]
enum NavItem {
    File {
        // Full relative path from the source root, e.g., "docs/about.md"
        rel_path: PathBuf,
        // The display name (e.g., "about.html")
        name: String, 
        is_current: bool,
    },
    Directory {
        // Full relative path from the source root, e.g., "docs"
        rel_path: PathBuf,
        // The display name (e.g., "docs")
        name: String,
        // Map of children, keyed by name for sorting
        children: BTreeMap<String, NavItem>, 
    },
}

#[derive(Debug)]
struct Args {
    source: PathBuf,
    target: PathBuf,
    verbose: bool,
}

type NavTree = BTreeMap<String, NavItem>; 

fn build_nav_tree(site_map: &SiteMap, current_rel_path: &Path) -> NavItem {
    let mut root_children: NavTree = BTreeMap::new();
    let current_html_path = current_rel_path.with_extension("html");
    
    let mut sorted_paths: Vec<PathBuf> = site_map.iter().cloned().collect();
    sorted_paths.sort();

    for rel_path in sorted_paths {
        if rel_path.file_name().map_or(false, |n| n == "template.html" || n == "styles.css") {
            continue;
        }

        let mut components = rel_path.components().peekable();
        // Start traversal from the root map
        let mut current_map = &mut root_children;
        let mut path_so_far = PathBuf::new();

        while let Some(component) = components.next() {
            let component_name = component.as_os_str().to_string_lossy().to_string();
            path_so_far.push(component_name.clone());

            let is_last_component = components.peek().is_none();
            
            if is_last_component {
                // This is a FILE (leaf node)
                let is_md = rel_path.extension().map_or(false, |ext| ext == "md");
                let file_name = if is_md {
                    rel_path.file_stem().unwrap_or_default().to_string_lossy().to_string() + ".html"
                } else {
                    rel_path.file_name().unwrap_or_default().to_string_lossy().to_string()
                };

                let item = NavItem::File {
                    rel_path: rel_path.clone(),
                    name: file_name,
                    is_current: rel_path.with_extension("html") == current_html_path,
                };
                
                current_map.insert(component_name, item);

            } else {
                // This is a DIRECTORY (branch node)

                // The .entry().or_insert_with() creates a temporary mutable reference
                // to the NavItem. We use a match to safely extract the `children` map
                // and reassign current_map to it.
                current_map = match current_map.entry(component_name.clone()).or_insert_with(|| {
                    NavItem::Directory {
                        rel_path: path_so_far.clone(),
                        name: component_name,
                        children: BTreeMap::new(),
                    }
                }) {
                    NavItem::Directory { children, .. } => children,
                    // This case should never happen if logic is correct, but required by match
                    NavItem::File {..} => panic!("Attempted to traverse into a file as if it were a directory!"), 
                };
            }
        }
    }

    NavItem::Directory {
        rel_path: PathBuf::new(),
        name: "Root".to_string(),
        children: root_children,
    }
}

fn nav_tree_to_html(
    nav_item: &NavItem,
    current_rel_path: &Path,
    site_map: &SiteMap,
    args: &Args,
    is_root: bool, // Set to true only for the initial call on the top-level Directory
) -> String {
    use NavItem::*;
    match nav_item {
        File { rel_path, name, is_current } => {
            let site_root_path = PathBuf::from("/").join(rel_path);
            let link_path = rewrite_link_to_relative(current_rel_path, &site_root_path, site_map, false);
            let title_attr = rel_path.to_string_lossy(); 

            if *is_current {
                 format!(
                    "<li class=\"current-file\" title=\"{}\">{}</li>",
                    title_attr, name
                )
            } else {
                format!(
                    "<li><a class=\"nav-link\" href=\"{}\" title=\"{}\">{}</a></li>",
                    link_path, title_attr, name
                )
            }
        }
        Directory { rel_path, name, children } => {
            let mut html = String::new();
            
            // 1. Determine the link for this directory's index page
            let index_link_path = {
                let site_root_path = if rel_path.as_os_str().is_empty() {
                    PathBuf::from("/index.md")
                } else {
                    PathBuf::from("/").join(rel_path).join("index.md")
                };
                rewrite_link_to_relative(current_rel_path, &site_root_path, site_map, false)
            };

            // 2. Start the List or Collapsible Directory Container
            // If it's the absolute root, just start the list.
            if is_root {
                html.push_str("<ul>");
            } else if !children.is_empty() {
                // If it's any other directory, wrap its content in a list item 
                // containing a collapsible <details> tag.
                
                // Determine if this directory is an ancestor of the current page.
                let is_open = current_rel_path.starts_with(rel_path);

                html.push_str("<li>");
                html.push_str(&format!(
                    "<details {}>", 
                    if is_open { "open" } else { "" }
                ));
                html.push_str(&format!(
                    "<summary><a href=\"{}\">{}</a></summary>",
                    index_link_path, name
                ));
                html.push_str("<ul>"); // Start the nested list inside <details>
            }

            // 3. Recursively Process Children (The Key Fix)
            // This loop must run regardless of whether it's the root or a nested directory,
            // as long as the directory has content.
            for (_, child) in children {
                // The recursive call handles the generation for files or nested directories.
                html.push_str(&nav_tree_to_html(child, current_rel_path, site_map, args, false));
            }
            
            // 4. Close the Containers
            if is_root {
                html.push_str("</ul>");
            } else if !children.is_empty() {
                // Close the nested <ul>, then the <details>, then the <li>
                html.push_str("</ul>"); 
                html.push_str("</details>");
                html.push_str("</li>");
            }

            html
        }
    }
}


/// A global map of all files to easily check for links.
type SiteMap = HashSet<PathBuf>;

const COLOR_RED: &str = "\x1b[31m";    
const COLOR_YELLOW: &str = "\x1b[33m"; 
const COLOR_CYAN: &str = "\x1b[36m";   
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

    fs::create_dir_all(&args.target)?;

    let html_template = match read_template(&args.source, &args) {
        Ok(template) => template,
        Err(e) => {
            print_error(&format!("Template Error: {}", e));
            return Err(e); 
        }
    };

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
        
    // Check for non-whitespace start (from previous turn)
    if !markdown_input.chars().next().map_or(true, char::is_whitespace) {
        print_warning(&format!(
            "File {} does not start with whitespace. This may cause the first heading to fail.", 
            path_rel.display()
        ));
    }

    // START NEW ADDITION: Check for the //...# pattern
    let lines: Vec<&str> = markdown_input.lines().collect();
    let mut is_in_comment_paragraph = false;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        
        if trimmed.starts_with("//") {
            // Line starts a comment paragraph
            is_in_comment_paragraph = true;
        } else if trimmed.is_empty() {
            // Blank line: if we're in a comment paragraph, the state continues.
            // We only check for the failure case below.
        } else if trimmed.starts_with("#") && is_in_comment_paragraph {
            // Found the failure pattern: //... followed by a heading on a new non-blank line
            print_warning(&format!(
                "File {} contains a potential parsing error on line {}: ATX Heading ('#') found immediately after a '//' comment block that was separated by only one blank line. This will be consumed as text.",
                path_rel.display(),
                i + 1 // Line number is 1-indexed
            ));
            
            // Reset the flag to avoid spamming the warning if multiple headings follow
            is_in_comment_paragraph = false;
        } else {
            // Any other non-blank line (like a regular paragraph, list, or valid block)
            // should reset the comment flag, as it closes the preceding paragraph.
            is_in_comment_paragraph = false;
        }
    }
    
    let mut options = Options::empty();
    
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
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
    // Build the nested tree structure from the flat site_map
    let nav_tree = build_nav_tree(site_map, current_rel_path);

    // Recursively convert the tree to nested HTML
    nav_tree_to_html(&nav_tree, current_rel_path, site_map, args, true)
}