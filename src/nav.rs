use std::{
    fs,
    io,
    path::{Path, PathBuf},
    collections::BTreeMap,
};
use pulldown_cmark::Parser;
use crate::config::{Args, NavItem, NavTree, SiteMap};
use crate::io::{collect_all_dirs_robust, print_error, print_info};
use crate::processing::{rewrite_link_to_relative, process_markdown_events, format_html_page, get_last_modified_date}; 

pub fn generate_navigation_html(args: &Args, site_map: &SiteMap, current_rel_path: &Path) -> String { 
    // Build the nested tree structure from the flat site_map
    let nav_tree = build_nav_tree(site_map, current_rel_path);

    // Recursively convert the tree to nested HTML
    nav_tree_to_html(&nav_tree, current_rel_path, site_map, args, true)
}

fn build_nav_tree(site_map: &SiteMap, current_rel_path: &Path) -> NavItem {
    let mut root_children: NavTree = BTreeMap::new();
    let current_html_path = current_rel_path.with_extension("html");
    
    let mut sorted_paths: Vec<PathBuf> = site_map.iter().cloned().collect();
    sorted_paths.sort(); 

    for rel_path in sorted_paths {
        // Exclude styles.css and template.html
        if rel_path.file_name().map_or(false, |n| n == "template.html" || n == "styles.css") {
            continue;
        }

        if rel_path.starts_with("scraps") || rel_path.starts_with("life-story") {
            continue;
        }

        let is_root_readme = rel_path.file_name().map_or(false, |n| n == "README.md")
            && rel_path.parent().map_or(true, |p| p.as_os_str().is_empty());
        
        if is_root_readme {
            continue;
        }

        // Convert path components to strings safely
        // FIX: Replaced .as_normal() (which doesn't exist) with pattern matching on the enum
        let components: Vec<String> = rel_path.components()
            .filter_map(|c| match c {
                std::path::Component::Normal(os_str) => Some(os_str.to_string_lossy().to_string()),
                _ => None,
            })
            .collect();

        if components.is_empty() { continue; }

        let file_name = components.last().unwrap().clone();
        
        // Start traversal from the root map
        let mut current_map = &mut root_children;
        let mut path_builder = PathBuf::new();

        // Iterate through all components except the last one (which is the file)
        for i in 0..components.len() - 1 {
            let dir_name = &components[i];
            path_builder.push(dir_name);

            // Get or create the Directory item
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

        // Insert the File item into the current map (which is now the parent directory's children)
        let is_current = rel_path.with_extension("html") == current_html_path;
        current_map.insert(file_name.clone(), NavItem::File {
            rel_path: rel_path.clone(),
            name: file_name,
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
            let title = if rel_dir_path.as_os_str().is_empty() {
                "Root Index".to_string()
            } else {
                rel_dir_path.to_string_lossy().to_string()
            };
            ("Index: ".to_string() + &title, String::new())
        };

        let (source_path_display, source_path_real) = if site_map.contains(&index_md_path) {
            (index_md_path.to_string_lossy().into_owned(), args.source.join(&index_md_path))
        } else {
            (rel_dir_path.to_string_lossy().into_owned(), args.source.join(&rel_dir_path))
        };
        
        let nav_rel_path = if site_map.contains(&index_md_path) {
            index_md_path.clone()
        } else {
            rel_dir_path.join("index.md") 
        };
        
        let nav_html = generate_navigation_html(args, site_map, &nav_rel_path);
        
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