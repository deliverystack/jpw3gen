use chrono::{DateTime, Utc};
use pulldown_cmark::{Options, Parser};
use regex::Regex;
use std::{collections::BTreeMap, fs, io, path::Path};

use crate::config::{Args, MetadataMap, PageMetadata, SiteMap};
use crate::html::{
    convert_urls_to_anchors, format_html_page, generate_breadcrumb_html, generate_canonical_url,
};
use crate::io::{print_error, print_info, print_warning};
use crate::markdown::{
    check_broken_links, normalize_markdown_content, prepare_content_for_parser,
    process_markdown_events,
};
use crate::nav::generate_navigation_html;

pub fn load_all_metadata_from_files(args: &Args, site_map: &SiteMap) -> io::Result<MetadataMap> {
    let mut metadata_map = BTreeMap::new();
    let json_regex = Regex::new(r"(?s)```json\s*(\{.*?\})\s*```\s*(\s*)$").unwrap();

    for rel_path in site_map
        .iter()
        .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
    {
        let path_source = args.source.join(rel_path);
        let markdown_input = fs::read_to_string(&path_source)?;
        let mut metadata = PageMetadata::default();

        if let Some(caps) = json_regex.captures(&markdown_input) {
            let json_str = &caps[1];
            match serde_json::from_str::<PageMetadata>(json_str) {
                Ok(parsed_meta) => metadata = parsed_meta,
                Err(e) => print_error(&format!(
                    "Failed to parse metadata in {}: {}",
                    rel_path.display(),
                    e
                )),
            }
        }

        let computed_title = {
            let content_without_json = json_regex
                .replace_all(&markdown_input, |caps: &regex::Captures| {
                    caps.get(2).map_or("", |m| m.as_str()).to_string()
                })
                .to_string();

            let parser = Parser::new(&content_without_json);
            let mut first_heading = String::new();
            let mut in_heading = false;

            for event in parser {
                match event {
                    pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading(..)) => {
                        in_heading = true;
                    }
                    pulldown_cmark::Event::Text(text) if in_heading => {
                        first_heading.push_str(&text);
                    }
                    pulldown_cmark::Event::End(pulldown_cmark::Tag::Heading(..)) if in_heading => {
                        break;
                    }
                    _ => {}
                }
            }

            if !first_heading.is_empty() {
                first_heading
            } else if let Some(page_title) = &metadata.page_title {
                page_title.clone()
            } else if let Some(nav_title) = &metadata.nav_title {
                nav_title.clone()
            } else {
                rel_path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| rel_path.to_string_lossy().to_string())
            }
        };

        metadata.computed_title = Some(computed_title);
        metadata_map.insert(rel_path.clone(), metadata);
    }

    Ok(metadata_map)
}

pub fn process_directory(
    args: &Args,
    site_map: &SiteMap,
    metadata_map: &MetadataMap,
    current_dir_source: &Path,
    html_template: &str,
) -> io::Result<()> {
    let current_dir_rel = current_dir_source
        .strip_prefix(&args.source)
        .unwrap_or(Path::new(""));
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

                let dir_rel_path = path_source
                    .strip_prefix(&args.source)
                    .unwrap_or(Path::new(""));
                let index_md_rel_path = dir_rel_path.join("index.md");

                let should_avoid = metadata_map
                    .get(&index_md_rel_path)
                    .and_then(|m| m.avoid_generation)
                    .unwrap_or(false);

                if should_avoid {
                    if args.verbose {
                        print_info(&format!(
                            "Skipping directory based on index.md metadata: {}",
                            dir_rel_path.display()
                        ));
                    }
                    continue;
                }
            }

            process_directory(args, site_map, metadata_map, &path_source, html_template)?;
        } else if path_source.is_file() {
            let file_name = path_source.file_name().unwrap_or_default();
            let path_target = current_dir_target.join(file_name);

            let rel_path = path_source
                .strip_prefix(&args.source)
                .unwrap_or(Path::new(""));

            let file_name_str = file_name.to_string_lossy().to_lowercase();

            const EXCLUDED_FILE_NAMES: [&str; 2] = ["template.html", "favicon.ico"];
            const EXCLUDED_EXTENSIONS: [&str; 5] = ["css", "js", "xml", "html", "ico"];

            if EXCLUDED_FILE_NAMES.contains(&file_name_str.as_str()) {
                if args.verbose {
                    print_info(&format!(
                        "Skipping explicitly excluded file: {}",
                        rel_path.display()
                    ));
                }
                continue;
            }

            if let Some(ext) = path_source
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
            {
                if EXCLUDED_EXTENSIONS.contains(&ext.as_str()) {
                    if args.verbose {
                        print_info(&format!(
                            "Skipping file with excluded extension (.{}): {}",
                            ext,
                            rel_path.display()
                        ));
                    }
                    continue;
                }
            }

            if rel_path.extension().is_some_and(|ext| ext == "md") {
                let metadata = metadata_map
                    .get(rel_path)
                    .expect("Metadata should exist for every markdown file in site_map");

                if metadata.avoid_generation.unwrap_or(false) {
                    if args.verbose {
                        print_info(&format!(
                            "Skipping file based on metadata: {}",
                            rel_path.display()
                        ));
                    }
                    continue;
                }

                markdown_to_html(
                    args,
                    site_map,
                    metadata,
                    &path_source,
                    &path_target,
                    rel_path,
                    html_template,
                    metadata_map,
                )?;
            } else {
                smart_copy_file(args, &path_source, &path_target, rel_path)?;
            }
        }
    }
    Ok(())
}

pub fn smart_copy_file(
    args: &Args,
    path_source: &Path,
    path_target: &Path,
    rel_path: &Path,
) -> io::Result<()> {
    if path_target.exists() {
        let source_content = fs::read(path_source)?;

        match fs::read(path_target) {
            Ok(target_content) => {
                if source_content == target_content {
                    if args.verbose {
                        print_info(&format!(
                            "Skipped (Unchanged Content): {}",
                            rel_path.display()
                        ));
                    }
                    return Ok(());
                }
            }
            Err(e) => return Err(e),
        }
    }

    fs::copy(path_source, path_target)?;
    if args.verbose {
        print_info(&format!(
            "Copied (Content Changed/New): {}",
            rel_path.display()
        ));
    }
    Ok(())
}

fn read_and_normalize_markdown(
    path_source: &Path,
    _path_rel: &Path,
    args: &Args,
) -> io::Result<String> {
    let markdown_input = fs::read_to_string(path_source).map_err(|e| {
        print_error(&format!(
            "Failed to read source file {}: {}",
            path_source.display(),
            e
        ));
        e
    })?;

    let (normalized_content, was_modified) =
        normalize_markdown_content(&markdown_input, path_source);

    if was_modified {
        fs::write(path_source, &normalized_content)?;
        print_warning(&format!(
            "Corrected source file (structural normalization): {}",
            path_source.display()
        ));
    } else if args.verbose {
        print_info(&format!(
            "Source file requires no structural modification: {}",
            path_source.display()
        ));
    }

    Ok(normalized_content)
}

fn parse_markdown_to_html(
    content: &str,
    metadata: &PageMetadata,
    args: &Args,
    site_map: &SiteMap,
    metadata_map: &MetadataMap,
    path_rel: &Path,
) -> (String, String) {
    let content_for_parser = prepare_content_for_parser(content, metadata);

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(&content_for_parser, options);
    process_markdown_events(args, site_map, metadata_map, parser, path_rel)
}

fn build_final_html(
    title: &str,
    path_rel: &Path,
    path_source: &Path,
    nav_html: &str,
    content: &str,
    html_template: &str,
    args: &Args,
    metadata_map: &MetadataMap,
) -> String {
    let date_created = get_creation_date(path_source);
    let last_modified_time = get_last_modified_date(path_source);

    let rel_path_str = {
        let path_str = path_rel.to_string_lossy();
        if path_str.starts_with('/') {
            path_str.into_owned()
        } else {
            format!("/{}", path_str)
        }
    };

    let final_content = convert_urls_to_anchors(content);

    let breadcrumb_html = generate_breadcrumb_html(path_rel, metadata_map, &args.base_url);
    let canonical_url = generate_canonical_url(path_rel, &args.base_url);

    format_html_page(
        title,
        &rel_path_str,
        &date_created,
        &last_modified_time,
        nav_html,
        &final_content,
        html_template,
        &breadcrumb_html,
        &canonical_url,
    )
}

fn should_skip_html_write(
    path_target_html: &Path,
    final_html: &str,
    path_rel: &Path,
    args: &Args,
) -> io::Result<bool> {
    if !path_target_html.exists() {
        return Ok(false);
    }

    match fs::read_to_string(path_target_html) {
        Ok(existing_content) => {
            if existing_content == final_html {
                if args.verbose {
                    print_info(&format!(
                        "Skipped (Unchanged HTML): {}",
                        path_rel.with_extension("html").display()
                    ));
                }
                Ok(true)
            } else {
                Ok(false)
            }
        }
        Err(e) => {
            print_warning(&format!(
                "Could not read target HTML for comparison {}: {}",
                path_target_html.display(),
                e
            ));
            Ok(false)
        }
    }
}

pub fn markdown_to_html(
    args: &Args,
    site_map: &SiteMap,
    metadata: &PageMetadata,
    path_source: &Path,
    path_target: &Path,
    path_rel: &Path,
    html_template: &str,
    metadata_map: &MetadataMap,
) -> io::Result<()> {
    let normalized_content = read_and_normalize_markdown(path_source, path_rel, args)?;

    check_broken_links(&normalized_content, path_source, path_rel);

    let (html_output_content, title_from_h1) = parse_markdown_to_html(
        &normalized_content,
        metadata,
        args,
        site_map,
        metadata_map,
        path_rel,
    );

    let title = metadata
        .page_title
        .as_ref()
        .unwrap_or(&title_from_h1)
        .clone();

    let nav_html = generate_navigation_html(args, site_map, metadata_map, path_rel);

    let final_html = build_final_html(
        &title,
        path_rel,
        path_source,
        &nav_html,
        &html_output_content,
        html_template,
        args,
        metadata_map,
    );

    let mut path_target_html = path_target.to_path_buf();
    path_target_html.set_extension("html");

    if should_skip_html_write(&path_target_html, &final_html, path_rel, args)? {
        return Ok(());
    }

    fs::write(&path_target_html, final_html)?;

    if args.verbose {
        print_info(&format!(
            "Converted: {} -> {}",
            path_rel.display(),
            path_rel.with_extension("html").display()
        ));
    }

    Ok(())
}

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

pub fn get_creation_date(path: &Path) -> String {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return "N/A".to_string(),
    };
    let created_time = match metadata.created() {
        Ok(t) => t,
        Err(_) => return "N/A".to_string(),
    };
    let datetime: DateTime<Utc> = created_time.into();
    datetime.format("%Y-%m-%d").to_string()
}
