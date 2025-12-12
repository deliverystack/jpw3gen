use std::{
    fs,
    io,
    path::{Path, PathBuf},
    collections::BTreeMap,
};
use regex::Regex; 
use pulldown_cmark::Parser;
use crate::config::{Args, NavItem, NavTree, SiteMap, MetadataMap, PageMetadata}; 
use crate::io::{collect_all_dirs_robust, print_error, print_info};
use crate::processing::{rewrite_link_to_relative, process_markdown_events, format_html_page, get_last_modified_date}; 

pub fn generate_navigation_html(args: &Args, site_map: &SiteMap, metadata_map: &MetadataMap, current_rel_path: &Path) -> String { 
    let nav_tree = build_nav_tree(site_map, metadata_map, current_rel_path);
    nav_tree_to_html(&nav_tree, current_rel_path, site_map, args, true)
}

fn build_nav_tree(site_map: &SiteMap, metadata_map: &MetadataMap, current_rel_path: &Path) -> NavItem {
    let mut root_children: NavTree = BTreeMap::new();
    let current_html_path = current_rel_path.with_extension("html");
    
    // Initial sort ensures consistent starting order for paths without explicit metadata
    let mut sorted_paths: Vec<PathBuf> = site_map.iter().cloned().collect();
    sorted_paths.sort(); 

    // Create a persistent default value outside the loop
    let default_metadata = PageMetadata::default();

    for rel_path in sorted_paths {
        let metadata = metadata_map.get(&rel_path).unwrap_or(&default_metadata);

        if metadata.exclude_from_nav.unwrap_or(false) {
            continue;
        }

        // Exclude specific files and directories
        //TODO: use JSON in index.md in these directories instead
        if rel_path.file_name().map_or(false, |n| n == "template.html" || n == "styles.css") ||
           rel_path.starts_with("scraps") || rel_path.starts_with("life-story") {
            continue;
        }

        //TODO: use JSON in readme instead
        let is_root_readme = rel_path.file_name().map_or(false, |n| n == "README.md")
            && rel_path.parent().map_or(true, |p| p.as_os_str().is_empty());
        
        if is_root_readme {
            continue;
        }

        // Convert path components to strings safely
        let components: Vec<String> = rel_path.components()
            .filter_map(|c| match c {
                std::path::Component::Normal(os_str) => Some(os_str.to_string_lossy().to_string()),
                _ => None,
            })
            .collect();

        if components.is_empty() { continue; }
        
        // 1. Determine the display name (used for rendering)
        let file_name = if let Some(title) = metadata.nav_title.clone() {
            title
        } else {
            rel_path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| components.last().unwrap().clone())
        };
        
        // 2. Determine the insertion key (used for sorting)
        let primary_sort_key = metadata.sort_key.as_ref().map(|s| s.to_string())
            .unwrap_or_else(|| file_name.clone());

        let final_sort_key_for_map = primary_sort_key.to_lowercase(); 

        // Create a unique, composite key: [sort_key OR display_name]--[unique_path]
        // The unique path remains case-sensitive for a truly stable tie-breaker.
        let insertion_key = format!("{}--{}", final_sort_key_for_map, rel_path.to_string_lossy());
        
        // Start traversal from the root map
        let mut current_map = &mut root_children;
        let mut path_builder = PathBuf::new();

        // Iterate through all components except the last one (the file)
        for i in 0..components.len() - 1 {
            let dir_name = &components[i];
            path_builder.push(dir_name);

            // Get or create the Directory item. Directory keys are still their names for simplicity
            let entry = current_map.entry(dir_name.clone()).or_insert_with(|| {
                 NavItem::Directory {
                     rel_path: path_builder.clone(),
                     name: dir_name.clone(),
                     children: BTreeMap::new(),
                 }
            });

            // Update reference to the children of the current directory
            current_map = entry.get_children_mut().expect("Item should be a directory");
        }

        // 3. Insert the File item using the composite insertion_key
        let is_current = rel_path.with_extension("html") == current_html_path;
        current_map.insert(insertion_key, NavItem::File {
            rel_path: rel_path.clone(),
            name: file_name, // <-- This is the value that is displayed
            is_current,
        });
    }

    NavItem::Directory {
        rel_path: PathBuf::new(),
        name: "Root".to_string(),
        children: root_children,
    }
}

fn nav_tree_to_html(nav_item: &NavItem, current_rel_path: &Path, site_map: &SiteMap, args: &Args, is_root: bool) -> String {
    use NavItem::*;
    match nav_item {
        File { rel_path, name, is_current } => {
            let site_root_path = PathBuf::from("/").join(rel_path);
            let link_path = rewrite_link_to_relative(current_rel_path, &site_root_path, site_map, false);
            let title_attr = rel_path.to_string_lossy(); 

            if *is_current {
                // Uses 'name' field, which holds the nav_title override
                format!("<li class=\"current-file\" title=\"{}\">{}</li>", title_attr, name)
            } else {
                // Uses 'name' field, which holds the nav_title override
                format!("<li><a class=\"nav-link\" href=\"{}\" title=\"{}\">{}</a></li>", link_path, title_attr, name)
            }
        }
        Directory { rel_path, name, children } => {
            let mut html = String::new();
            
            // Determine the link for this directory's index page
            let index_link_path = {
                let site_root_path = if rel_path.as_os_str().is_empty() {
                    PathBuf::from("/index.md")
                } else {
                    PathBuf::from("/").join(rel_path).join("index.md")
                };
                rewrite_link_to_relative(current_rel_path, &site_root_path, site_map, false)
            };

            if is_root {
                html.push_str("<ul>");
            } else if !children.is_empty() {
                let is_open = current_rel_path.starts_with(rel_path);
                html.push_str("<li>");
                html.push_str(&format!("<details {}>", if is_open { "open" } else { "" }));
                html.push_str(&format!("<summary><a href=\"{}\">{}</a></summary>", index_link_path, name));
                html.push_str("<ul>");
            }

            // Process Directories first
            let mut has_directories = false;
            // Iterate over children, which are already sorted by the BTreeMap key (the composite sort key)
            for (_, child) in children.iter() {
                if let NavItem::Directory { .. } = child {
                    html.push_str(&nav_tree_to_html(child, current_rel_path, site_map, args, false));
                    has_directories = true;
                }
            }
            
            // Check for files to add separator
            let has_files = children.iter().any(|(_, child)| matches!(child, NavItem::File { .. }));
            
            if has_directories && has_files {
                html.push_str("<li class=\"nav-separator\"></li>");
            }
            
            // Process Files
            for (_, child) in children.iter() {
                if let NavItem::File { .. } = child {
                    html.push_str(&nav_tree_to_html(child, current_rel_path, site_map, args, false));
                }
            }
            
            if is_root {
                html.push_str("</ul>");
            } else if !children.is_empty() {
                html.push_str("</ul>"); 
                html.push_str("</details>");
                html.push_str("</li>");
            }

            html
        }
    }
}

pub fn generate_all_index_files(args: &Args, site_map: &SiteMap, metadata_map: &MetadataMap, html_template: &str) -> io::Result<()> {
    let dirs_to_index = collect_all_dirs_robust(&args.source)?;
    let mut sorted_dirs: Vec<PathBuf> = dirs_to_index.into_iter().collect();
    sorted_dirs.sort();
    
    // Create a persistent default value outside the loop
    let default_index_metadata = PageMetadata::default();
    
    // Regex is initialized once outside the loop
    let json_regex = Regex::new(r"(?s)```json\s*(\{.*?\})\s*```\s*(\s*)$").unwrap();

    for rel_dir_path in sorted_dirs {
        let index_md_path = rel_dir_path.join("index.md");
        let path_target_dir = args.target.join(&rel_dir_path);
        let path_target = path_target_dir.join("index.html");

        // Check if an index.md exists and get its metadata
        let has_index_md = site_map.contains(&index_md_path);
        // Use the reference to the persistent default value
        let index_metadata = metadata_map.get(&index_md_path).unwrap_or(&default_index_metadata);

        // Check for avoidance flag
        if has_index_md && index_metadata.avoid_generation.unwrap_or(false) {
            if args.verbose {
                print_info(&format!("Skipped (Avoid Generation): {}", index_md_path.display()));
            }
            continue;
        }

        let (title, content) = if has_index_md {
            let path_source = args.source.join(&index_md_path);
            let markdown_input = fs::read_to_string(&path_source)?;
            
            // Temporary stripping for parser only
            let content_without_json = json_regex.replace_all(&markdown_input, |caps: &regex::Captures| {
                caps.get(2).map_or("", |m| m.as_str()).to_string()
            }).to_string();

            let parser = Parser::new(&content_without_json);
            let (html_output, title_from_h1) = process_markdown_events(args, site_map, parser, &index_md_path);
            
            // Use metadata override for the title
            let final_title = index_metadata.page_title.as_ref().unwrap_or(&title_from_h1).clone();
            (final_title, html_output)
        } else {
            let default_title = if rel_dir_path.as_os_str().is_empty() {
                "Root Index".to_string()
            } else {
                rel_dir_path.to_string_lossy().to_string()
            };
            ("Index: ".to_string() + &default_title, String::new())
        };

        // Ensure source_path_display starts with a leading slash (/)
        let source_path_rel_str = if has_index_md {
            index_md_path.to_string_lossy().into_owned()
        } else {
            rel_dir_path.to_string_lossy().into_owned()
        };
        
        let source_path_display = if source_path_rel_str.is_empty() {
            // Case 1: Root directory path (e.g., "" becomes "/")
            "/".to_string()
        } else {
            // Case 2: Any other path (e.g., "docs/file.md" becomes "/docs/file.md")
            format!("/{}", source_path_rel_str)
        };

        let source_path_real = if has_index_md {
            args.source.join(&index_md_path)
        } else {
            args.source.join(&rel_dir_path)
        };
        
        // Use the index.md path for navigation context if it exists, otherwise a synthetic path
        let nav_rel_path = if has_index_md {
            index_md_path.clone()
        } else {
            rel_dir_path.join("index.md") 
        };
        
        // Pass metadata_map to navigation generation
        let nav_html = generate_navigation_html(args, site_map, metadata_map, &nav_rel_path);
        
        let last_modified = get_last_modified_date(&source_path_real);
        let default_content = if content.is_empty() {
            format!("<h1>{}</h1><p>No <code>index.md</code> file found. Displaying directory index.</p>", title)
        } else {
            content
        };

        let final_html = format_html_page(
            &title, 
            &source_path_display, 
            &last_modified,
            &nav_html, 
            &default_content, 
            html_template
        );

        fs::create_dir_all(&path_target_dir)?;
        
         if path_target.exists() {
            if let Ok(existing_content) = fs::read_to_string(&path_target) {
                if existing_content == final_html {
                    if args.verbose {
                        print_info(&format!("Skipped (Unchanged Index HTML): {}", path_target.display()));
                    }
                    continue;
                }
            }
        }

        match fs::write(&path_target, final_html) {
            Ok(_) => {
                if args.verbose {
                    print_info(&format!("Successfully generated index.html at: {}", path_target.display()));
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