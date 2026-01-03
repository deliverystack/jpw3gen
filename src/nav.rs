use crate::config::{Args, MetadataMap, NavItem, NavTree, PageMetadata, SiteMap};
use crate::html::{format_html_page, generate_breadcrumb_html, generate_canonical_url};
use crate::io::{collect_all_dirs, print_error, print_info};
use crate::markdown::{process_markdown_events, rewrite_link_to_relative};
use crate::processing::{get_creation_date, get_last_modified_date};
use pulldown_cmark::Parser;
use regex::Regex;
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

pub fn generate_navigation_html(
    args: &Args,
    site_map: &SiteMap,
    metadata_map: &MetadataMap,
    current_rel_path: &Path,
) -> String {
    let nav_tree = build_nav_tree(site_map, metadata_map, current_rel_path);
    nav_tree_to_html(&nav_tree, current_rel_path, site_map, args, true)
}

fn get_directory_sort_keys(metadata_map: &MetadataMap) -> BTreeMap<PathBuf, String> {
    let mut dir_sort_keys = BTreeMap::new();

    for (path, metadata) in metadata_map.iter() {
        if path.file_name().is_some_and(|n| n == "index.md") {
            if let Some(sort_key) = &metadata.sort_key {
                if let Some(parent_dir) = path.parent() {
                    dir_sort_keys.insert(parent_dir.to_path_buf(), sort_key.to_lowercase());
                }
            }
        }
    }

    dir_sort_keys
}

fn build_nav_tree(
    site_map: &SiteMap,
    metadata_map: &MetadataMap,
    current_rel_path: &Path,
) -> NavItem {
    let mut root_children: NavTree = BTreeMap::new();
    let current_html_path = current_rel_path.with_extension("html");

    let dir_sort_keys = get_directory_sort_keys(metadata_map);

    let mut sorted_paths: Vec<PathBuf> = site_map.iter().cloned().collect();
    sorted_paths.sort();

    let default_metadata = PageMetadata::default();

    let mut excluded_dirs = std::collections::HashSet::new();

    for rel_path in site_map
        .iter()
        .filter(|p| p.file_name().is_some_and(|n| n == "index.md"))
    {
        let metadata = metadata_map.get(rel_path).unwrap_or(&default_metadata);

        if metadata.exclude_from_nav.unwrap_or(false) {
            if let Some(parent_dir) = rel_path.parent() {
                excluded_dirs.insert(parent_dir.to_path_buf());
            }
        }
    }

    for rel_path in sorted_paths {
        let metadata = metadata_map.get(&rel_path).unwrap_or(&default_metadata);

        if metadata.exclude_from_nav.unwrap_or(false) {
            continue;
        }

        let is_in_excluded_dir = excluded_dirs.iter().any(|excluded_dir| {
            !excluded_dir.as_os_str().is_empty() && rel_path.starts_with(excluded_dir)
        });

        if is_in_excluded_dir {
            continue;
        }

        let file_name = rel_path.file_name().unwrap_or_default();
        let file_name_str = file_name.to_string_lossy().to_lowercase();

        const EXCLUDED_FILE_NAMES: [&str; 2] = ["template.html", "favicon.ico"];
        const EXCLUDED_EXTENSIONS: [&str; 5] = ["css", "js", "xml", "html", "json"];

        if EXCLUDED_FILE_NAMES.contains(&file_name_str.as_str()) {
            continue;
        }

        if let Some(ext) = rel_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
        {
            if EXCLUDED_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }
        }

        let is_index_md = rel_path.file_name().is_some_and(|n| n == "index.md");
        let is_root = rel_path.parent().is_none_or(|p| p.as_os_str().is_empty());

        if is_index_md && !is_root {
            continue;
        }

        let components: Vec<String> = rel_path
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(os_str) => Some(os_str.to_string_lossy().to_string()),
                _ => None,
            })
            .collect();

        if components.is_empty() {
            continue;
        }

        let file_name = if let Some(title) = metadata.nav_title.clone() {
            title
        } else {
            rel_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| components.last().unwrap().clone())
        };

        let primary_sort_key = metadata
            .sort_key
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| file_name.clone());

        let final_sort_key_for_map = primary_sort_key.to_lowercase();

        let insertion_key = format!("{}--{}", final_sort_key_for_map, rel_path.to_string_lossy());

        let mut current_map = &mut root_children;
        let mut path_builder = PathBuf::new();
        let mut is_at_root_level = true;

        for i in 0..components.len() - 1 {
            let dir_name_str = &components[i];
            path_builder.push(dir_name_str);

            is_at_root_level = false;

            let dir_sort_key = dir_sort_keys
                .get(&path_builder)
                .cloned()
                .unwrap_or_else(|| dir_name_str.to_lowercase());

            let entry = current_map.entry(dir_sort_key).or_insert_with(|| {
                let index_md_path = path_builder.join("index.md");

                let mut dir_display_name = dir_name_str.clone();

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
            current_map = entry
                .get_children_mut()
                .expect("Item should be a directory");
        }

        if !is_at_root_level || components.len() == 1 {
            let is_current = rel_path.with_extension("html") == current_html_path;
            current_map.insert(
                insertion_key,
                NavItem::File {
                    rel_path: rel_path.clone(),
                    name: file_name,
                    is_current,
                },
            );
        }
    }

    NavItem::Directory {
        rel_path: PathBuf::new(),
        name: "Root".to_string(),
        children: root_children,
    }
}

fn should_render_branch(item_path: &Path, current_path: &Path) -> bool {
    if item_path.components().count() <= 1 {
        return true;
    }

    if current_path.starts_with(item_path) {
        return true;
    }

    if let (Some(item_parent), Some(current_parent)) = (item_path.parent(), current_path.parent()) {
        if item_parent == current_parent {
            return true;
        }
    }

    false
}

fn nav_tree_to_html(
    nav_item: &NavItem,
    current_rel_path: &Path,
    site_map: &SiteMap,
    args: &Args,
    is_root: bool,
) -> String {
    use NavItem::*;
    match nav_item {
        File {
            rel_path,
            name,
            is_current,
        } => {
            let site_root_path = PathBuf::from("/").join(rel_path);
            let link_path =
                rewrite_link_to_relative(current_rel_path, &site_root_path, site_map, false);
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
        Directory {
            rel_path,
            name,
            children,
        } => {
            let mut html = String::new();

            if !is_root && !should_render_branch(rel_path, current_rel_path) {
                let site_root_path = if rel_path.as_os_str().is_empty() {
                    PathBuf::from("/index.md")
                } else {
                    PathBuf::from("/").join(rel_path).join("index.md")
                };
                let index_link_path =
                    rewrite_link_to_relative(current_rel_path, &site_root_path, site_map, false);

                return format!(
                    "<li><details><summary><a href=\"{}\">{}</a></summary></details></li>",
                    index_link_path, name
                );
            }

            let index_link_path = {
                let site_root_path = if rel_path.as_os_str().is_empty() {
                    PathBuf::from("/index.md")
                } else {
                    PathBuf::from("/").join(rel_path).join("index.md")
                };
                rewrite_link_to_relative(current_rel_path, &site_root_path, site_map, false)
            };

            let current_html_path = current_rel_path.with_extension("html");
            let is_open =
                current_rel_path.starts_with(rel_path) && !rel_path.as_os_str().is_empty();
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
                html.push_str(&format!(
                    "<summary><a{} href=\"{}\">{}</a></summary>",
                    summary_class, index_link_path, name
                ));
                html.push_str("<ul>");
            }

            let mut has_directories = false;
            for (_, child) in children.iter() {
                if let NavItem::Directory { .. } = child {
                    html.push_str(&nav_tree_to_html(
                        child,
                        current_rel_path,
                        site_map,
                        args,
                        false,
                    ));
                    has_directories = true;
                }
            }

            let has_files = children
                .iter()
                .any(|(_, child)| matches!(child, NavItem::File { .. }));

            if has_directories && has_files {
                html.push_str("<li class=\"nav-separator\"></li>");
            }

            for (_, child) in children.iter() {
                if let NavItem::File { .. } = child {
                    html.push_str(&nav_tree_to_html(
                        child,
                        current_rel_path,
                        site_map,
                        args,
                        false,
                    ));
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

pub fn generate_all_index_files(
    args: &Args,
    site_map: &SiteMap,
    metadata_map: &MetadataMap,
    html_template: &str,
) -> io::Result<()> {
    let dirs_to_index = collect_all_dirs(&args.source)?;
    let mut sorted_dirs: Vec<PathBuf> = dirs_to_index.into_iter().collect();
    sorted_dirs.sort();

    let default_index_metadata = PageMetadata::default();

    let json_regex = Regex::new(r"(?s)```json\s*(\{.*?\})\s*```\s*(\s*)$").unwrap();

    for rel_dir_path in sorted_dirs {
        let index_md_path = rel_dir_path.join("index.md");
        let path_target_dir = args.target.join(&rel_dir_path);
        let path_target = path_target_dir.join("index.html");

        let has_index_md = site_map.contains(&index_md_path);
        let index_metadata = metadata_map
            .get(&index_md_path)
            .unwrap_or(&default_index_metadata);

        if has_index_md && index_metadata.avoid_generation.unwrap_or(false) {
            if args.verbose {
                print_info(&format!(
                    "Skipped (Avoid Generation): {}",
                    index_md_path.display()
                ));
            }
            continue;
        }

        let (title, content) = if has_index_md {
            let path_source = args.source.join(&index_md_path);
            let markdown_input = fs::read_to_string(&path_source)?;

            let content_without_json = json_regex
                .replace_all(&markdown_input, |caps: &regex::Captures| {
                    caps.get(2).map_or("", |m| m.as_str()).to_string()
                })
                .to_string();

            let parser = Parser::new(&content_without_json);
            let (html_output, title_from_h1) =
                process_markdown_events(args, site_map, metadata_map, parser, &index_md_path);

            let final_title = index_metadata
                .page_title
                .as_ref()
                .unwrap_or(&title_from_h1)
                .clone();
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

        let date_created = get_creation_date(&source_path_real);
        let last_modified = get_last_modified_date(&source_path_real);
        let default_content = if content.is_empty() {
            format!(
                "<h1>{}</h1><p>Use the links on the left to access content.</p>",
                title
            )
        } else {
            content
        };

        let breadcrumb_html = generate_breadcrumb_html(&nav_rel_path, metadata_map, &args.base_url);
        let canonical_url = generate_canonical_url(&nav_rel_path, &args.base_url);

        let final_html = format_html_page(
            &title,
            &source_path_display,
            &date_created,
            &last_modified,
            &nav_html,
            &default_content,
            html_template,
            &breadcrumb_html,
            &canonical_url,
        );

        fs::create_dir_all(&path_target_dir)?;

        if path_target.exists() {
            if let Ok(existing_content) = fs::read_to_string(&path_target) {
                if existing_content == final_html {
                    if args.verbose {
                        print_info(&format!(
                            "Skipped (Unchanged Index HTML): {}",
                            path_target.display()
                        ));
                    }
                    continue;
                }
            }
        }

        match fs::write(&path_target, final_html) {
            Ok(_) => {
                if args.verbose {
                    print_info(&format!(
                        "Successfully generated index.html at: {}",
                        path_target.display()
                    ));
                }
            }
            Err(e) => {
                print_error(&format!(
                    "Failed to write index.html to {}: {}",
                    path_target.display(),
                    e
                ));
                return Err(e);
            }
        }
    }

    Ok(())
}
