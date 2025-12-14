use std::{
    fs,
    io,
    path::{Path, PathBuf},
    collections::BTreeMap
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

// Helper 1/4: Determines which directories should be excluded based on their index.md metadata.
fn get_excluded_directories(site_map: &SiteMap, metadata_map: &MetadataMap) -> std::collections::HashSet<PathBuf> {
    let default_metadata = PageMetadata::default();
    let mut excluded_dirs = std::collections::HashSet::new();
    
    for rel_path in site_map.iter().filter(|p| p.file_name().map_or(false, |n| n == "index.md")) {
        let metadata = metadata_map.get(rel_path).unwrap_or(&default_metadata);
        
        if metadata.exclude_from_nav.unwrap_or(false) {
            if let Some(parent_dir) = rel_path.parent() {
                excluded_dirs.insert(parent_dir.to_path_buf()); 
            }
        }
    }
    excluded_dirs
}

// Helper 2/4: Consolidates all logic for filtering/skipping a path.
fn should_skip_path(rel_path: &Path, metadata: &PageMetadata, excluded_dirs: &std::collections::HashSet<PathBuf>) -> bool {
    // 1. Metadata Exclusion
    if metadata.exclude_from_nav.unwrap_or(false) {
        return true;
    }

    // 2. Directory Exclusion Check
    let is_in_excluded_dir = excluded_dirs.iter().any(|excluded_dir| {
        !excluded_dir.as_os_str().is_empty() && rel_path.starts_with(excluded_dir)
    });

    if is_in_excluded_dir {
        return true;
    }

    // 3. index.md/README.md Exclusion
    let is_index_md = rel_path.file_name().map_or(false, |n| n == "index.md");
    let is_root = rel_path.parent().map_or(true, |p| p.as_os_str().is_empty()); 

    // Always exclude non-root index.md files from appearing as "Files" in the tree
    if is_index_md && !is_root {
        return true;
    }
    
    let is_root_readme = rel_path.file_name().map_or(false, |n| n == "README.md")
        && rel_path.parent().map_or(true, |p| p.as_os_str().is_empty());
    
    if is_root_readme {
        return true;
    }

    // 4. File Extension/Name Exclusion (Includes favicon.ico, styles.css, etc.)
    let file_name = rel_path.file_name().map_or("", |n| n.to_str().unwrap_or(""));
        
    if file_name == "favicon.ico" || // Exclude favicon.ico
        file_name == "template.html" ||
        file_name.ends_with(".css") || // Exclude CSS files
        file_name.ends_with(".js") || // Exclude JS files
        rel_path.starts_with("scraps") || 
        rel_path.starts_with("life-story") {
        return true;
    }
    
    // 5. Only process markdown files here for navigation content
    let path_extension = rel_path.extension().map_or("", |ext| ext.to_str().unwrap_or(""));
    if path_extension != "md" {
        return true;
    }
    
    false
}

// Helper 3/4: Determines the display name and sort key for a given path.
// Returns (file_name, insertion_key)
fn create_nav_item_data(rel_path: &Path, metadata: &PageMetadata) -> Option<(String, String)> {
    let components: Vec<String> = rel_path.components()
        .filter_map(|c| match c {
            std::path::Component::Normal(os_str) => Some(os_str.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();

    if components.is_empty() { return None; }
    
    // 1. Determine the display name (used for rendering FILES)
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

    let insertion_key = format!("{}--{}", final_sort_key_for_map, rel_path.to_string_lossy());

    Some((file_name, insertion_key))
}

// Helper 4/4: Traverses the NavTree structure and inserts the NavItem at the correct location.
fn insert_item_into_tree(
    root_children: &mut NavTree, 
    rel_path: &Path, 
    metadata_map: &MetadataMap, 
    current_html_path: &Path,
    file_name: String, 
    insertion_key: String
) {
    let components: Vec<String> = rel_path.components()
        .filter_map(|c| match c {
            std::path::Component::Normal(os_str) => Some(os_str.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();

    let mut current_map = root_children;
    let mut path_builder = PathBuf::new();
    let mut is_at_root_level = true; 
    // Removed the unused `default_metadata` declaration from here.

    // Iterate through all components except the last one (the file)
    for i in 0..components.len() - 1 {
        let dir_name_str = &components[i];
        path_builder.push(dir_name_str);
        
        is_at_root_level = false;

        // Get or create the Directory item. 
        let entry = current_map.entry(dir_name_str.clone()).or_insert_with(|| {
             let index_md_path = path_builder.join("index.md");
             
             // Default to the directory folder name
             let mut dir_display_name = dir_name_str.clone();

             // Check if we have metadata for this directory's index.md
             if let Some(dir_metadata) = metadata_map.get(&index_md_path) {
                 if let Some(title) = &dir_metadata.nav_title {
                     dir_display_name = title.clone();
                 }
             }

             NavItem::Directory {
                 rel_path: path_builder.clone(),
                 name: dir_display_name,
                 children: BTreeMap::new(),
             }
        });

        // Update reference to the children of the current directory
        current_map = entry.get_children_mut().expect("Item should be a directory");
    }

    // Insert the File item using the composite insertion_key
    if !is_at_root_level || components.len() == 1 {
        let is_current = rel_path.with_extension("html") == *current_html_path;
        current_map.insert(insertion_key, NavItem::File {
            // FIX: Convert &Path to PathBuf by calling to_path_buf()
            rel_path: rel_path.to_path_buf(),
            name: file_name,
            is_current,
        });
    }
}


fn build_nav_tree(site_map: &SiteMap, metadata_map: &MetadataMap, current_rel_path: &Path) -> NavItem {
    let mut root_children: NavTree = BTreeMap::new();
    let current_html_path = current_rel_path.with_extension("html");
    
    // Initial sort ensures consistent starting order
    let mut sorted_paths: Vec<PathBuf> = site_map.iter().cloned().collect();
    sorted_paths.sort(); 

    // Create a persistent default value outside the loop
    // FIX: Renamed to satisfy the unused variable warning
    let _default_metadata = PageMetadata::default();

    // 1. Determine excluded directories
    let excluded_dirs = get_excluded_directories(site_map, metadata_map);

    for rel_path_buf in sorted_paths {
        let rel_path = rel_path_buf.as_path(); // Use &Path for filtering/data creation
        // FIX: Updated usage to the new variable name
        let metadata = metadata_map.get(rel_path).unwrap_or(&_default_metadata);

        // 2. Filter out paths that should be skipped
        if should_skip_path(rel_path, metadata, &excluded_dirs) {
            continue;
        }

        // 3. Create display name and sort key
        let (file_name, insertion_key) = match create_nav_item_data(rel_path, metadata) {
            Some(data) => data,
            None => continue,
        };

        // 4. Traverse and insert the item into the tree
        insert_item_into_tree(
            &mut root_children, 
            rel_path, 
            metadata_map, 
            &current_html_path, 
            file_name, 
            insertion_key
        );
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
                format!("<li class=\"current-file\" title=\"{}\">{}</li>", title_attr, name)
            } else {
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

            let current_html_path = current_rel_path.with_extension("html");
            let is_open = current_rel_path.starts_with(rel_path) && !rel_path.as_os_str().is_empty();
            let index_html_path = rel_path.join("index.md").with_extension("html");
            let is_current_page = current_html_path == index_html_path;
            
            let li_class = if is_open || is_current_page { 
                " class=\"current-branch\"" 
            } else { 
                "" 
            };
            
            let summary_class = if is_current_page { 
                " class=\"current-summary\"" 
            } else { 
                "" 
            };

            if is_root {
                html.push_str("<ul>");
            } else if !children.is_empty() {
                html.push_str(&format!("<li{}>", li_class)); 
                html.push_str(&format!("<details {}>", if is_open { "open" } else { "" }));
                // Uses the updated 'name' which now contains nav_title
                html.push_str(&format!("<summary><a{} href=\"{}\">{}</a></summary>", summary_class, index_link_path, name)); 
                html.push_str("<ul>");
            }

            // Process Directories first
            let mut has_directories = false;
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
    
    let default_index_metadata = PageMetadata::default();
    
    let json_regex = Regex::new(r"(?s)```json\s*(\{.*?\})\s*```\s*(\s*)$").unwrap();

    for rel_dir_path in sorted_dirs {
        let index_md_path = rel_dir_path.join("index.md");
        let path_target_dir = args.target.join(&rel_dir_path);
        let path_target = path_target_dir.join("index.html");

        let has_index_md = site_map.contains(&index_md_path);
        let index_metadata = metadata_map.get(&index_md_path).unwrap_or(&default_index_metadata);

        if has_index_md && index_metadata.avoid_generation.unwrap_or(false) {
            if args.verbose {
                print_info(&format!("Skipped (Avoid Generation): {}", index_md_path.display()));
            }
            continue;
        }

        let (title, content) = if has_index_md {
            let path_source = args.source.join(&index_md_path);
            let markdown_input = fs::read_to_string(&path_source)?;
            
            let content_without_json = json_regex.replace_all(&markdown_input, |caps: &regex::Captures| {
                caps.get(2).map_or("", |m| m.as_str()).to_string()
            }).to_string();

            let parser = Parser::new(&content_without_json);
            let (html_output, title_from_h1) = process_markdown_events(args, site_map, parser, &index_md_path);
            
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

        let source_path_rel_str = if has_index_md {
            index_md_path.to_string_lossy().into_owned()
        } else {
            rel_dir_path.to_string_lossy().into_owned()
        };
        
        let source_path_display = if source_path_rel_str.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", source_path_rel_str)
        };

        let source_path_real = if has_index_md {
            args.source.join(&index_md_path)
        } else {
            args.source.join(&rel_dir_path)
        };
        
        let nav_rel_path = if has_index_md {
            index_md_path.clone()
        } else {
            rel_dir_path.join("index.md") 
        };
        
        let nav_html = generate_navigation_html(args, site_map, metadata_map, &nav_rel_path);
        
        let last_modified = get_last_modified_date(&source_path_real);
        let default_content = if content.is_empty() {
            format!("<h1>{}</h1><p>Use the links on the left to access content.</p>", title)
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