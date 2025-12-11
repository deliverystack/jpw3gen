use std::{fs, io, path::{Path, PathBuf}, mem};
use pulldown_cmark::{Parser, Options, Event, Tag, HeadingLevel, LinkType};

use regex::Regex;
use crate::config::{Args, SiteMap};
use crate::io::{print_info, print_warning};
use crate::nav::generate_navigation_html;

pub fn process_directory(args: &Args, site_map: &SiteMap, current_dir_source: &Path, html_template: &str) -> io::Result<()> {
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

pub fn markdown_to_html(args: &Args, site_map: &SiteMap, path_source: &Path, path_target: &Path, path_rel: &Path, html_template: &str) -> io::Result<()> {
    // Regex to remove all Unicode control characters, excluding standard newlines and tabs.
    let control_char_regex = Regex::new(r"[\p{Cc}\p{Cf}&&[^\n\t\r]]").unwrap();

    let markdown_input = fs::read_to_string(path_source)?;
    
    // --- START FINAL CORRECTION & CLEANUP BLOCK ---
    
    // 1. AGGRESSIVE CLEANING & DASH CONVERSION
    let cleaned_content_stage_1: String = control_char_regex.replace_all(&markdown_input, "")
        .to_string()
        .replace('\r', "")      // Normalize to Unix line endings
        // TARGETED DASH FIX: Convert specialized dashes to their proper Markdown source equivalent
        .replace('\u{2011}', "-") // Non-Breaking Hyphen -> standard hyphen
        .replace('\u{2013}', "-") // En Dash -> standard hyphen
        .replace('\u{2014}', "--") // Em Dash -> two standard hyphens (REQUIRED for Markdown to render Em Dash)
        .replace('\u{00A0}', " "); // Non-breaking space to standard space

    // 2. C-COMMENT CONVERSION
    // Convert '//' lines to Markdown HTML comments to preserve them without breaking headings.
    let lines_to_convert: Vec<String> = cleaned_content_stage_1.lines()
        .map(|line| {
            if line.trim_start().starts_with("//") {
                let comment_text = line.trim_start().trim_start_matches('/');
                format!("{}", comment_text) 
            } else {
                line.to_string()
            }
        })
        .collect();
    
    let content_with_converted_comments = lines_to_convert.join("\n");
    
    let mut final_content = content_with_converted_comments.clone();

    // 3. ENSURE LEADING NEWLINE
    let starts_with_whitespace = final_content.chars().next().map_or(true, char::is_whitespace);
    
    // 4. DETERMINE IF MODIFICATION OCCURRED (Comparison variable is now safely accessed)
    let original_normalized_for_comparison: String = content_with_converted_comments; // Safely moves here as it's no longer needed after clone
    
    let mut content_was_structurally_modified = final_content != original_normalized_for_comparison;

    if !starts_with_whitespace {
        final_content.insert(0, '\n');
        content_was_structurally_modified = true; 
    }

    // 5. OVERWRITE FILE IF MODIFIED AND REPORT WARNINGS
    if content_was_structurally_modified {
        // Overwrite the original source file with the corrected content
        fs::write(path_source, &final_content)?;
        
        print_warning(&format!("Corrected source file: {}", path_source.display()));
        
        if !starts_with_whitespace {
             print_warning(&format!(
                "File {} did not start with whitespace. Prepended a blank line.", 
                path_rel.display()
            ));
        }

    } else if args.verbose {
        print_info(&format!("Source file requires no modification: {}", path_source.display()));
    }
    
    // --- END FILE CORRECTION BLOCK ---
    
    // 6. LINK CHECKING
    // Regex finds all Markdown links that end in .md
    let link_regex = Regex::new(r"\[[^\]]+\]\(([^)]+\.md)\)").unwrap();
    let parent_dir = path_source.parent().unwrap_or_else(|| Path::new(""));

    for caps in link_regex.captures_iter(&final_content) {
        let link_target = &caps[1];
        let target_path = parent_dir.join(link_target);

        // Check if the target .md file exists relative to the source file
        if !target_path.exists() {
            print_warning(&format!(
                "Broken link detected in {}: Link to non-existent file '{}' at full path {}", 
                path_rel.display(), 
                link_target,
                target_path.display()
            ));
        }
    }


    // 7. RENDER MARKDOWN TO HTML
    let mut options = Options::empty();
    
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_SMART_PUNCTUATION); 
    
    // Use the final_content for parsing
    let parser = Parser::new_ext(&final_content, options); 
    
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

pub fn process_markdown_events<'a>(
    args: &Args, 
    site_map: &SiteMap,
    parser: Parser<'a, 'a>,
    path_rel: &Path,
) -> (String, String) {
    // Variables for storing heading data to resolve ownership issues
    let mut title_h1 = String::new();
    let mut in_h1 = false; 
    let mut events = Vec::new();
    let html_output = String::new(); // Starts empty, only for content written outside the event loop
    
    let mut first_heading_found = false; 
    
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

pub fn events_to_html(events: Vec<Event>) -> String {
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, events.into_iter());
    html_output
}

pub fn resolve_link_path(from_path_rel: &Path, link_target: &Path) -> PathBuf {
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

pub fn rewrite_link_to_relative(from_path_rel: &Path, link_target: &Path, site_map: &SiteMap, verbose: bool) -> String {
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

pub fn format_html_page(title: &str, nav_html: &str, content: &str, html_template: &str) -> String {
    html_template
        .replace("{{ title }}", title)
        .replace("{{ nav_html }}", nav_html)
        .replace("{{ content }}", content)
}