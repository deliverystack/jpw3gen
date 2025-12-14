use std::{fs, io, path::{Path, PathBuf}, mem, collections::BTreeMap}; 
use pulldown_cmark::{Parser, Options, Event, Tag, HeadingLevel, LinkType};
use chrono::{DateTime, Utc};
use serde_json; 

use regex::Regex;
use crate::config::{Args, SiteMap, PageMetadata, MetadataMap}; 
use crate::io::{print_info, print_warning, print_error};
use crate::nav::generate_navigation_html;

// NEW FUNCTION: Loads and parses metadata from all markdown files.
pub fn load_all_metadata_from_files(args: &Args, site_map: &SiteMap) -> io::Result<MetadataMap> {
    let mut metadata_map = BTreeMap::new();
    let json_regex = Regex::new(r"(?s)```json\s*(\{.*?\})\s*```\s*(\s*)$").unwrap();

    for rel_path in site_map.iter().filter(|p| p.extension().map_or(false, |ext| ext == "md")) {
        let path_source = args.source.join(rel_path);
        let markdown_input = fs::read_to_string(&path_source)?;
        let mut metadata = PageMetadata::default();

        if let Some(caps) = json_regex.captures(&markdown_input) {
            let json_str = &caps[1];
            match serde_json::from_str::<PageMetadata>(json_str) {
                Ok(parsed_meta) => metadata = parsed_meta,
                Err(e) => print_error(&format!("Failed to parse metadata in {}: {}", rel_path.display(), e)),
            }
        }
        metadata_map.insert(rel_path.clone(), metadata);
    }
    Ok(metadata_map)
}

pub fn process_directory(args: &Args, site_map: &SiteMap, metadata_map: &MetadataMap, current_dir_source: &Path, html_template: &str) -> io::Result<()> {
    let current_dir_rel = current_dir_source.strip_prefix(&args.source).unwrap_or(Path::new(""));
    let current_dir_target = args.target.join(current_dir_rel);

    fs::create_dir_all(&current_dir_target)?;

    for entry in fs::read_dir(current_dir_source)? {
        let entry = entry?;
        let path_source = entry.path();

        if path_source.is_dir() {
            if let Some(name) = path_source.file_name().and_then(|s| s.to_str()) {
                if name.starts_with('.') {
                    continue; // Keep hard-coded hidden directory exclusion
                }
                
                // Check directory exclusion via index.md metadata
                let dir_rel_path = path_source.strip_prefix(&args.source).unwrap_or(Path::new(""));
                let index_md_rel_path = dir_rel_path.join("index.md");

                // Check if index.md exists and has 'avoid_generation: true'
                let should_avoid = metadata_map
                    .get(&index_md_rel_path)
                    .and_then(|m| m.avoid_generation)
                    .unwrap_or(false); // Default to false

                if should_avoid {
                    if args.verbose {
                        print_info(&format!("Skipping directory based on index.md metadata: {}", dir_rel_path.display()));
                    }
                    continue; 
                }
            }
            process_directory(args, site_map, metadata_map, &path_source, html_template)?;
        } else if path_source.is_file() {
            let file_name = path_source.file_name().unwrap_or_default();
            let path_target = current_dir_target.join(file_name);
            
            let rel_path = path_source.strip_prefix(&args.source).unwrap_or(Path::new(""));

            // NEW GLOBAL FILE EXCLUSION LOGIC
            let file_name_str = file_name.to_string_lossy().to_lowercase();
            
            // Explicit list of files to skip copying/processing - see a similar list in nav.rs - and related logic in jpw3gen.sh
            const EXCLUDED_FILE_NAMES: [&str; 2] = ["template.html", "favicon.ico"];

            // Explicit list of extensions to skip copying/processing (CSS, JS, generated HTML, sitemap XML)  - see a similar list in nav.rs - and related logic in jpw3gen.sh
            const EXCLUDED_EXTENSIONS: [&str; 5] = ["css", "js", "xml", "html", "ico"]; 

            if EXCLUDED_FILE_NAMES.contains(&file_name_str.as_str()) {
                if args.verbose {
                    print_info(&format!("Skipping explicitly excluded file: {}", rel_path.display()));
                }
                continue;
            }

            if let Some(ext) = path_source.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()) {
                if EXCLUDED_EXTENSIONS.contains(&ext.as_str()) {
                    if args.verbose {
                        print_info(&format!("Skipping file with excluded extension (.{}: {})", ext, rel_path.display()));
                    }
                    continue;
                }
            }
            // END NEW GLOBAL FILE EXCLUSION LOGIC
            
            if rel_path.extension().map_or(false, |ext| ext == "md") {
                let metadata = metadata_map.get(rel_path).expect("Metadata should exist for every markdown file in site_map");
                
                // Check file exclusion via its own metadata
                if metadata.avoid_generation.unwrap_or(false) {
                    if args.verbose {
                        print_info(&format!("Skipping file based on metadata: {}", rel_path.display()));
                    }
                    continue; // Skip the file if metadata says to avoid generation
                }

                markdown_to_html(args, site_map, metadata, &path_source, &path_target, rel_path, html_template, metadata_map)?;
            } else {
                smart_copy_file(args, &path_source, &path_target, rel_path)?;
            }
        }
    }
    Ok(())
}

pub fn smart_copy_file(args: &Args, path_source: &Path, path_target: &Path, rel_path: &Path) -> io::Result<()> {
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

pub fn markdown_to_html(args: &Args, site_map: &SiteMap, metadata: &PageMetadata, path_source: &Path, path_target: &Path, path_rel: &Path, html_template: &str, metadata_map: &MetadataMap) -> io::Result<()> {
    let control_char_regex = Regex::new(r"[\p{Cc}\p{Cf}&&[^\n\t\r]]").unwrap();
    let json_regex = Regex::new(r"(?s)```json\s*(\{.*?\})\s*```\s*(\s*)$").unwrap();
    let todo_regex = Regex::new(r"^(?P<prefix>[\s*>\-\+]*)(TODO:?\s*)(?P<text>.*)$").unwrap();
    
    // Group 1 captures the preceding context (space, '(', or start of line).
    // Group 2 captures the URL.
    let bare_url_regex = Regex::new(r"(\s|\(|^)(https?://\S+)").unwrap();


    let markdown_input = match fs::read_to_string(path_source) {
        Ok(c) => c,
        Err(e) => {
            print_error(&format!("Failed to read source file {}: {}", path_source.display(), e));
            return Err(e);
        }
    };
    
    // This variable holds content that needs structural cleanup (written back to disk)
    let mut content_for_normalization = markdown_input.clone();

    // 1. CLEANING & DASH CONVERSION 
    content_for_normalization = control_char_regex.replace_all(&content_for_normalization, "")
        .to_string()
        .replace('\r', "")      
        .replace('\u{2011}', "-")   // Non-breaking hyphen
        .replace('\u{2013}', "-")   // En dash
        .replace('\u{2014}', "--")  // Em dash
        .replace('\u{00A0}', " ")   // Non-breaking space
        // Smart quotes to straight quotes
        .replace('\u{2018}', "'")   // Left single quote
        .replace('\u{2019}', "'")   // Right single quote
        .replace('\u{201A}', "'")   // Single low quote
        .replace('\u{201B}', "'")   // Single high reversed quote
        .replace('\u{201C}', "\"")  // Left double quote
        .replace('\u{201D}', "\"")  // Right double quote
        .replace('\u{201E}', "\"")  // Double low quote
        .replace('\u{201F}', "\"")  // Double high reversed quote
        // Ellipsis
        .replace('\u{2026}', "...")  // Horizontal ellipsis
        // Other common replacements
        .replace('\u{2032}', "'")   // Prime (often used as apostrophe)
        .replace('\u{2033}', "\"")  // Double prime
        .replace('\u{2010}', "-");  // Hyphen
        
    // 2. C-COMMENT CONVERSION
    let lines_to_convert: Vec<String> = content_for_normalization.lines()
        .map(|line| {
            // Ensure we don't accidentally comment out the JSON block
            if !line.contains("```json") && !line.contains("```") && line.trim_start().starts_with("//") {
                let comment_text = line.trim_start().trim_start_matches('/');
                format!("{}", comment_text) 
            } else {
                line.to_string()
            }
        })
        .collect();
    
    content_for_normalization = lines_to_convert.join("\n");
    
    // 3. TODO FORMATTING
    let lines_with_todo: Vec<String> = content_for_normalization.lines()
        .map(|line| {
            if todo_regex.is_match(line) {
                 todo_regex.replace(line, "$prefix***//TODO: $text***").to_string()
            } else {
                line.to_string()
            }
        })
        .collect();
    
    content_for_normalization = lines_with_todo.join("\n");


    // 4. ENSURE LEADING NEWLINE
    let starts_with_whitespace = content_for_normalization.chars().next().map_or(true, char::is_whitespace);
    
    // Use the original markdown_input to check if the file was structurally modified
    let mut content_was_structurally_modified = content_for_normalization != markdown_input;

    if !starts_with_whitespace {
        content_for_normalization.insert(0, '\n');
        content_was_structurally_modified = true; 
    }

    // 5. OVERWRITE FILE IF MODIFIED (Saves the normalized content, including JSON)
    // ONLY content_for_normalization (which contains only structural/non-linking changes) is written back.
    if content_was_structurally_modified {
        fs::write(path_source, &content_for_normalization)?;
        print_warning(&format!("Corrected source file (structural normalization): {}", path_source.display()));
        if !starts_with_whitespace {
             print_warning(&format!("File {} did not start with whitespace. Prepended a blank line.", path_rel.display()));
        }
    } else if args.verbose {
        print_info(&format!("Source file requires no structural modification: {}", path_source.display()));
    }
    
    
    // 6. IN-MEMORY URL FIXING FOR PARSER ONLY
    // We clone the content_for_normalization to create the version for the parser.
    let mut content_for_parser = content_for_normalization.clone();
    
    // Use the closure replacement to avoid wrapping URLs that are part of an explicit link [text](url)
    content_for_parser = bare_url_regex
        .replace_all(&content_for_parser, |caps: &regex::Captures| {
            let preceding_context = &caps[1];
            let url = &caps[2];

            if preceding_context == "(" {
                // If preceded by '(', it's part of an explicit MD link. Return match unchanged.
                format!("{}{}", preceding_context, url)
            } else {
                // Otherwise, it's a bare URL (preceded by whitespace or start of line).
                // FIX: Correctly wrap the URL in autolink syntax (<URL>).
                format!("{}<{}>", preceding_context, url)
            }
        })
        .to_string();

    
    // 7. JSON METADATA STRIPPING FOR PARSER ONLY (Controlled by metadata flag)
    if !metadata.keep_json_in_content.unwrap_or(false) {
        // If flag is false or missing, strip the JSON block from the content passed to the parser.
        content_for_parser = json_regex.replace_all(&content_for_parser, |caps: &regex::Captures| {
            caps.get(2).map_or("", |m| m.as_str()).to_string()
        }).to_string();
    }


    // 8. LINK CHECKING (Unchanged - uses content_for_normalization for line/file structure consistency)
    let parent_dir = path_source.parent().unwrap_or_else(|| Path::new(""));
    
    // Check for internal broken links
    let link_regex = Regex::new(r"\[[^\]]+\]\(([^)]+\.md)\)").unwrap();
    for caps in link_regex.captures_iter(&content_for_normalization) {
        let link_target = &caps[1];
        let target_path = parent_dir.join(link_target);
        if !target_path.exists() {
            print_warning(&format!("Broken link detected in {}: Link to non-existent file '{}'", path_rel.display(), link_target));
        }
    }
    
    // Check for broken image links
    let image_link_regex = Regex::new(r"!\[[^\]]*\]\(([^)]+\.(png|jpe?g|gif|svg))\)").unwrap();
    for caps in image_link_regex.captures_iter(&content_for_normalization) {
        let link_target = &caps[1]; 
        let target_path = parent_dir.join(link_target);
        if !target_path.exists() {
            print_warning(&format!("Broken image link detected in {}: Link to non-existent image '{}'", path_rel.display(), link_target));
        }
    }


    // 9. RENDER MARKDOWN TO HTML (Uses the modified, stripped in-memory content_for_parser)
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    
    let parser = Parser::new_ext(&content_for_parser, options); 
    
    let (html_output_content, title_from_h1) = process_markdown_events(args, site_map, parser, path_rel);

    // Use metadata override for the title
    let title = metadata.page_title.as_ref().unwrap_or(&title_from_h1).clone();

    let mut path_target_html = path_target.to_path_buf();
    path_target_html.set_extension("html");
    
    let nav_html = generate_navigation_html(args, site_map, metadata_map, path_rel);
    
    let last_modified_time = get_last_modified_date(path_source);
    
    // Ensure rel_path_str (for {{ source_path }}) starts with a leading slash (/)
    let rel_path_str = {
        let path_str = path_rel.to_string_lossy();
        if path_str.starts_with('/') {
            path_str.into_owned()
        } else {
            format!("/{}", path_str)
        }
    };

    // Use the potentially overridden title
    let final_html = format_html_page(&title, &rel_path_str, &last_modified_time, &nav_html, &html_output_content, html_template);
    
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

pub fn process_markdown_events<'a>(
    args: &Args, 
    site_map: &SiteMap,
    parser: Parser<'a, 'a>,
    path_rel: &Path,
) -> (String, String) {
    let mut title_h1 = String::new();
    let mut in_h1 = false; 
    let mut events = Vec::new();
    let html_output = String::new(); 
    
    let mut first_heading_found = false; 
    let mut _current_heading_level: Option<HeadingLevel> = None; 
    let mut current_heading_id: Option<String> = None;
    let mut current_heading_classes: Option<Vec<String>> = None; 
    
    // Link tracking flag
    let mut in_link = false; 

    for event in parser {
        match event {
            Event::Start(Tag::Heading(level, id, classes_from_event)) => {
                _current_heading_level = Some(level); 
                current_heading_id = id.map(|s| s.to_string());
                
                if !classes_from_event.is_empty() {
                    let owned_classes = classes_from_event.clone().into_iter().map(|s| s.to_string()).collect();
                    current_heading_classes = Some(owned_classes);
                } else {
                    current_heading_classes = None;
                }

                if !first_heading_found {
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
                    mem::take(&mut current_heading_id);
                    mem::take(&mut current_heading_classes);
                    events.push(Event::Html("</h1>".into()));
                } else {
                    events.push(Event::End(Tag::Heading(level, id, classes)));
                    mem::take(&mut current_heading_id);
                    mem::take(&mut current_heading_classes);
                }
                mem::take(&mut _current_heading_level);
            }
            Event::Text(text) => {
                if in_h1 {
                    title_h1.push_str(&text);
                } 
                
                // If inside an existing link, skip any processing
                if in_link {
                    events.push(Event::Text(text));
                    continue; 
                }

                // Custom auto-linking logic is now handled in markdown_to_html via regex substitution.
                events.push(Event::Text(text));
            }
            Event::Start(Tag::Link(link_type, dest, title_attr)) => {
                in_link = true; 
                let is_external = dest.starts_with("http") || dest.starts_with("ftp");
                if link_type == LinkType::Inline && !is_external {
                    let dest_path = PathBuf::from(&*dest);
                    let new_dest = rewrite_link_to_relative(path_rel, &dest_path, site_map, args.verbose);
                    events.push(Event::Start(Tag::Link(link_type, new_dest.into(), title_attr)));
                } else if is_external {
                    // If it is an external link, we use a custom target="_blank" HTML tag
                    // FIX: Remove the title attribute insertion
                    let html_tag_start = format!("<a href=\"{}\" target=\"_blank\">", dest);
                    events.push(Event::Html(html_tag_start.into()));
                } else {
                    events.push(Event::Start(Tag::Link(link_type, dest, title_attr)));
                }
            }
            Event::End(Tag::Link(link_type, dest, title_attr)) => {
                in_link = false; 
                let is_external = dest.starts_with("http") || dest.starts_with("ftp");
                if is_external {
                    events.push(Event::Html("</a>".into()));
                } else {
                    events.push(Event::End(Tag::Link(link_type, dest, title_attr)));
                }
            }
            e => events.push(e),
        }
    }

    let final_title = if !title_h1.is_empty() { title_h1 } else { path_rel.to_string_lossy().to_string() };
    let html_from_events = events_to_html(events);
    let final_content = html_output + &convert_urls_to_anchors(&html_from_events);    
    (final_content, final_title)
}

//TODO: What does this do
pub fn events_to_html(events: Vec<Event>) -> String {
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, events.into_iter());
    html_output
}

//TODO: What does this do
pub fn resolve_link_path(from_path_rel: &Path, link_target: &Path) -> PathBuf {
    if link_target.to_string_lossy().starts_with('/') {
        return link_target.to_path_buf();
    }
    
    let from_dir = from_path_rel.parent().unwrap_or(Path::new(""));
    let resolved_path = from_dir.join(link_target);
    
    let mut components = Vec::new();
    for component in resolved_path.components() {
        match component {
            std::path::Component::ParentDir => { components.pop(); }
            std::path::Component::Normal(name) => { components.push(name); }
            _ => {}
        }
    }
    PathBuf::from("/").join(components.iter().collect::<PathBuf>())
}

//TODO: What does this do
pub fn rewrite_link_to_relative(from_path_rel: &Path, link_target: &Path, site_map: &SiteMap, verbose: bool) -> String {
    let root_rel_path = resolve_link_path(from_path_rel, link_target);
    let target_path_rel = root_rel_path.strip_prefix("/").unwrap_or(Path::new(""));
    let mut final_target_path = target_path_rel.to_path_buf();
    
    if target_path_rel.extension().map_or(false, |ext| ext == "md") {
        final_target_path.set_extension("html");
    } else if target_path_rel.is_dir() || target_path_rel.extension().is_none() || target_path_rel.to_string_lossy().is_empty() {
        let target_is_index_md = target_path_rel.join("index.md");
        if target_path_rel.as_os_str().is_empty() || site_map.contains(&target_is_index_md) {
             final_target_path = target_path_rel.join("index.html");
        }
    }

    let current_dir = from_path_rel.parent().unwrap_or(Path::new(""));
    let rel_path = pathdiff::diff_paths(&final_target_path, current_dir).unwrap_or(final_target_path.clone()); 
    let rel_path_str = rel_path.to_string_lossy();
    
    if verbose {
        print_info(&format!("Link rewrite: {} -> {} (via {})", link_target.display(), rel_path_str, from_path_rel.display()));
    }
    rel_path_str.to_string()
}

/// Replace tokens in the HTML with values
pub fn format_html_page(title: &str, rel_path_str: &str, last_modified_time: &str, nav_html: &str, content: &str, html_template: &str) -> String {
    html_template
        .replace("{{ title }}", title)
        .replace("{{ header_title }}", title) 
        .replace("{{ source_path }}", rel_path_str) 
        .replace("{{ last_modified }}", last_modified_time) 
        .replace("{{ nav_html }}", nav_html)
        .replace("{{ content }}", content)
}

/// Retrieve the last modification date for the specified file.
pub fn get_last_modified_date(path: &Path) -> String {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return "N/A".to_string(),
    };
    let modified_time = match metadata.modified() {
        Ok(t) => t,
        Err(_) => return "N/A".to_string(),
    };
    let datetime: DateTime<Utc> = modified_time.into();
    datetime.format("%Y-%m-%d").to_string()
}

pub fn generate_sitemap_xml(args: &Args, metadata_map: &MetadataMap) -> io::Result<()> {
    let sitemap_path = args.target.join("sitemap.xml");
    
    // Default values if metadata is missing
    let default_changefreq = "monthly";
    // Base priority (0.5 is a common default for pages not explicitly prioritized)
    let base_priority = 0.5; 

    let mut entries = Vec::new();
    
    for (rel_path, metadata) in metadata_map.iter() {
        // Only include files explicitly marked for sitemap
        if metadata.include_in_sitemap.unwrap_or(false) {
            
            // 1. Determine the URL (loc)
            let mut url_path = rel_path.to_path_buf();
            
            // Per the requirement, use /directory rather than /directory/index.html
            if rel_path.file_name().map_or(false, |n| n == "index.md") {
                // If index.md is at root (e.g., index.md), result is /
                if rel_path.parent().map_or(false, |p| p.as_os_str().is_empty()) {
                    url_path = PathBuf::from(""); // Represents the root URL "/"
                } else {
                    // If index.md is in a directory (e.g., docs/index.md), result is /docs
                    url_path = rel_path.parent().unwrap().to_path_buf();
                }
            } else {
                // For other files (e.g., page.md), result is /page.html
                url_path.set_extension("html");
            }
            
            // Final URL string, ensuring a leading slash for non-root paths
            let loc_url = {
                let path_str = url_path.to_string_lossy();
                if path_str.is_empty() {
                    "/".to_string()
                } else {
                    format!("/{}", path_str)
                }
            };
            
            // 2. Determine last modified date (lastmod)
            let source_path = args.source.join(rel_path);
            let last_mod = get_last_modified_date(&source_path); 
            
            // 3. Determine change frequency (changefreq)
            let change_freq = metadata.sitemap_changefreq.as_deref().unwrap_or(default_changefreq);
            
            // 4. Determine priority (priority)
            // Use the new dedicated key, falling back to the base priority if not set.
            let priority_float = metadata.sitemap_priority.unwrap_or(base_priority).min(1.0).max(0.0);
            let priority = format!("{:.1}", priority_float); // Format to one decimal place

            // Create the XML entry
            let entry = format!(
                "  <url>\n    <loc>{}</loc>\n    <lastmod>{}</lastmod>\n    <changefreq>{}</changefreq>\n    <priority>{}</priority>\n  </url>",
                loc_url,
                last_mod,
                change_freq,
                priority
            );
            entries.push(entry);
        }
    }

    if entries.is_empty() {
        print_warning("No files marked for sitemap.xml generation.");
        return Ok(());
    }

    let xml_content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n{}\n</urlset>",
        entries.join("\n")
    );

    // ... (rest of the file writing logic remains the same)
    match fs::write(&sitemap_path, xml_content) {
        Ok(_) => {
            if args.verbose {
                print_info(&format!("Successfully generated sitemap.xml at: {}", sitemap_path.display()));
            }
        }
        Err(e) => {
            print_error(&format!("Failed to write sitemap.xml: {}", e));
            return Err(e);
        }
    }
    
    Ok(())
}

pub fn convert_urls_to_anchors(html: &str) -> String {
    let url_regex = Regex::new(r"https?://[^\s<]+").unwrap();
    let anchor_regex = Regex::new(r"<a\b[^>]*>.*?</a>").unwrap();
    
    let mut result = String::new();
    let mut last_pos = 0;
    
    // Find all anchor tags to skip
    let mut anchor_ranges = Vec::new();
    for mat in anchor_regex.find_iter(html) {
        anchor_ranges.push((mat.start(), mat.end()));
    }
    
    // Process URLs outside of anchor tags
    for url_match in url_regex.find_iter(html) {
        let start = url_match.start();
        let end = url_match.end();
        
        // Check if this URL is inside an anchor tag
        let in_anchor = anchor_ranges.iter().any(|(a_start, a_end)| start >= *a_start && end <= *a_end);
        
        if !in_anchor {
            // Add text before the URL
            result.push_str(&html[last_pos..start]);
            // Add the URL as an anchor
            let url = url_match.as_str();
            result.push_str(&format!("<a href=\"{}\">{}</a>", url, url));
            last_pos = end;
        }
    }
    
    // Add remaining text
    result.push_str(&html[last_pos..]);
    
    if result.is_empty() {
        html.to_string()
    } else {
        result
    }
}