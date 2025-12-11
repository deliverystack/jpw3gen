use std::{
    fs,
    io,
    path::{Path, PathBuf},
    collections::BTreeMap,
};
use pulldown_cmark::Parser;
use crate::config::{Args, NavItem, NavTree, SiteMap};
use crate::io::{collect_all_dirs_robust, print_error, print_info, print_warning};
use crate::processing::{rewrite_link_to_relative, process_markdown_events, format_html_page};

pub fn generate_navigation_html(args: &Args, site_map: &SiteMap, current_rel_path: &Path) -> String { 
    // Build the nested tree structure from the flat site_map
    let nav_tree = build_nav_tree(site_map, current_rel_path);

    // Recursively convert the tree to nested HTML
    nav_tree_to_html(&nav_tree, current_rel_path, site_map, args, true)
}

fn build_nav_tree(site_map: &SiteMap, current_rel_path: &Path) -> NavItem {
    let mut root_children: NavTree = BTreeMap::new();
    let current_html_path = current_rel_path.with_extension("html");
    
    // The BTreeMap handles alphabetical sorting of keys (file/dir names).
    let mut sorted_paths: Vec<PathBuf> = site_map.iter().cloned().collect();
    sorted_paths.sort(); // Maintain basic alphabetical sort on full paths

    for rel_path in sorted_paths {
        // Exclude styles.css and template.html (existing logic)
        if rel_path.file_name().map_or(false, |n| n == "template.html" || n == "styles.css") {
            continue;
        }

        // 🚨 EXCLUSION: Exclude README.md (which becomes README.html) ONLY from the root.
        // The root README.md has no parent directory component (parent() returns empty path).
        let is_root_readme = rel_path.file_name().map_or(false, |n| n == "README.md")
            && rel_path.parent().map_or(true, |p| p.as_os_str().is_empty());
        
        if is_root_readme {
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
                current_map = match current_map.entry(component_name.clone()).or_insert_with(|| {
                    NavItem::Directory {
                        rel_path: path_so_far.clone(),
                        name: component_name,
                        children: BTreeMap::new(),
                    }
                }) {
                    NavItem::Directory { children, .. } => children,
                    // This case should never happen if logic is correct
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
            if is_root {
                html.push_str("<ul>");
            } else if !children.is_empty() {
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

            // 3. Recursively Process Children (NEW: Directory-First)
            
            // Pass A: Directories
            for (_, child) in children.iter() {
                if let NavItem::Directory { .. } = child {
                    html.push_str(&nav_tree_to_html(child, current_rel_path, site_map, args, false));
                }
            }
            
            // Pass B: Files
            for (_, child) in children.iter() {
                if let NavItem::File { .. } = child {
                    html.push_str(&nav_tree_to_html(child, current_rel_path, site_map, args, false));
                }
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

pub fn generate_all_index_files(args: &Args, site_map: &SiteMap, html_template: &str) -> io::Result<()> {
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
            // print_warning(&format!("No index.md found in directory: {}", rel_dir_path.display()));
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
//                    println!("Generated index.html for: {}", rel_dir_path.display());
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