use pulldown_cmark::{Event, HeadingLevel, LinkType, Parser, Tag};
use regex::Regex;
use std::{
    mem,
    path::{Path, PathBuf},
};

use crate::config::{Args, MetadataMap, SiteMap};
use crate::io::{print_info, print_warning};

pub fn normalize_markdown_content(content: &str, _path: &Path) -> (String, bool) {
    let control_char_regex = Regex::new(r"[\p{Cc}\p{Cf}&&[^\n\t\r]]").unwrap();
    let todo_regex = Regex::new(r"^(?P<prefix>[\s*>\-\+]*)(TODO:?\s*)(?P<text>.*)$").unwrap();

    let mut normalized = content.to_string();
    let original = content.to_string();

    normalized = control_char_regex
        .replace_all(&normalized, "")
        .to_string()
        .replace('\r', "")
        .replace(['\u{2011}', '\u{2013}'], "-")
        .replace('\u{2014}', "--")
        .replace('\u{00A0}', " ")
        .replace(['\u{2018}', '\u{2019}', '\u{201A}', '\u{201B}'], "'")
        .replace(['\u{201C}', '\u{201D}', '\u{201E}', '\u{201F}'], "\"")
        .replace('\u{2026}', "...")
        .replace('\u{2032}', "'")
        .replace('\u{2033}', "\"")
        .replace('\u{2010}', "-");

    let lines_to_convert: Vec<String> = normalized
        .lines()
        .map(|line| {
            if !line.contains("```json")
                && !line.contains("```")
                && line.trim_start().starts_with("//")
            {
                let comment_text = line.trim_start().trim_start_matches('/');
                comment_text.to_string()
            } else {
                line.to_string()
            }
        })
        .collect();

    normalized = lines_to_convert.join("\n");

    let lines_with_todo: Vec<String> = normalized
        .lines()
        .map(|line| {
            if todo_regex.is_match(line) {
                todo_regex
                    .replace(line, "$prefix***//TODO: $text***")
                    .to_string()
            } else {
                line.to_string()
            }
        })
        .collect();

    normalized = lines_with_todo.join("\n");

    let starts_with_whitespace = normalized.chars().next().is_none_or(char::is_whitespace);

    let mut was_modified = normalized != original;

    if !starts_with_whitespace {
        normalized.insert(0, '\n');
        was_modified = true;
    }

    (normalized, was_modified)
}

pub fn prepare_content_for_parser(content: &str, metadata: &crate::config::PageMetadata) -> String {
    let bare_url_regex = Regex::new(r"(\s|\(|^)(https?://\S+)").unwrap();
    let json_regex = Regex::new(r"(?s)```json\s*(\{.*?\})\s*```\s*(\s*)$").unwrap();

    let mut prepared = content.to_string();

    prepared = bare_url_regex
        .replace_all(&prepared, |caps: &regex::Captures| {
            let preceding_context = &caps[1];
            let url = &caps[2];

            if preceding_context == "(" {
                format!("{}{}", preceding_context, url)
            } else {
                format!("{}<{}>", preceding_context, url)
            }
        })
        .to_string();

    if !metadata.keep_json_in_content.unwrap_or(false) {
        prepared = json_regex
            .replace_all(&prepared, |caps: &regex::Captures| {
                caps.get(2).map_or("", |m| m.as_str()).to_string()
            })
            .to_string();
    }

    prepared
}

pub fn check_broken_links(content: &str, source_path: &Path, rel_path: &Path) {
    let link_regex = Regex::new(r"\[[^\]]+\]\(([^):]+\.md)\)").unwrap();
    let image_link_regex = Regex::new(r"!\[[^\]]*\]\(([^)]+\.(png|jpe?g|gif|svg))\)").unwrap();

    let parent_dir = source_path.parent().unwrap_or_else(|| Path::new(""));

    for caps in link_regex.captures_iter(content) {
        let link_target = &caps[1];
        let target_path = parent_dir.join(link_target);
        if !target_path.exists() {
            print_warning(&format!(
                "Broken link detected in {}: Link to non-existent file '{}'",
                rel_path.display(),
                link_target
            ));
        }
    }

    for caps in image_link_regex.captures_iter(content) {
        let link_target = &caps[1];
        let target_path = parent_dir.join(link_target);
        if !target_path.exists() {
            print_warning(&format!(
                "Broken image link detected in {}: Link to non-existent image '{}'",
                rel_path.display(),
                link_target
            ));
        }
    }
}

pub fn process_markdown_events<'a>(
    args: &Args,
    site_map: &SiteMap,
    metadata_map: &MetadataMap,
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

    let mut in_link = false;
    let mut current_link_dest: Option<String> = None;
    let mut link_text_events: Vec<Event> = Vec::new();
    let mut should_auto_title = false;

    for event in parser {
        match event {
            Event::Start(Tag::Heading(level, id, classes_from_event)) => {
                _current_heading_level = Some(level);
                current_heading_id = id.map(|s| s.to_string());

                if !classes_from_event.is_empty() {
                    let owned_classes = classes_from_event
                        .clone()
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect();
                    current_heading_classes = Some(owned_classes);
                } else {
                    current_heading_classes = None;
                }

                if !first_heading_found {
                    first_heading_found = true;
                    in_h1 = true;
                    events.push(Event::Start(Tag::Heading(
                        HeadingLevel::H1,
                        id,
                        classes_from_event,
                    )));
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

                if in_link {
                    let trimmed = text.trim();
                    if trimmed == "{title}" || trimmed == "{TITLE}" {
                        should_auto_title = true;
                    }
                    link_text_events.push(Event::Text(text.clone()));
                    continue;
                }

                events.push(Event::Text(text));
            }
            Event::Start(Tag::Link(link_type, dest, title_attr)) => {
                in_link = true;
                link_text_events.clear();
                should_auto_title = false;

                let is_external = dest.starts_with("http") || dest.starts_with("ftp");
                if link_type == LinkType::Inline && !is_external {
                    let dest_path = PathBuf::from(&*dest);
                    current_link_dest = Some(dest.to_string());
                    let new_dest =
                        rewrite_link_to_relative(path_rel, &dest_path, site_map, args.verbose);
                    events.push(Event::Start(Tag::Link(
                        link_type,
                        new_dest.into(),
                        title_attr,
                    )));
                } else if is_external {
                    current_link_dest = Some(dest.to_string());
                    let html_tag_start = format!(
                        "<a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">",
                        dest
                    );
                    events.push(Event::Html(html_tag_start.into()));
                } else {
                    current_link_dest = None;
                    events.push(Event::Start(Tag::Link(link_type, dest, title_attr)));
                }
            }
            Event::End(Tag::Link(link_type, dest, title_attr)) => {
                let is_external = dest.starts_with("http") || dest.starts_with("ftp");

                let link_is_empty = link_text_events.is_empty();

                if should_auto_title || link_is_empty {
                    if let Some(original_dest) = &current_link_dest {
                        if is_external {
                            events.push(Event::Text(original_dest.clone().into()));
                        } else {
                            let dest_path = PathBuf::from(original_dest);
                            if let Some(auto_title) = get_link_title(
                                path_rel,
                                &dest_path,
                                metadata_map,
                                site_map,
                                args.verbose,
                            ) {
                                events.push(Event::Text(auto_title.into()));
                            } else {
                                events.append(&mut link_text_events);
                            }
                        }
                    } else {
                        events.append(&mut link_text_events);
                    }
                } else {
                    // Check if link text is just the file path - if so, replace with title
                    let link_text = link_text_events
                        .iter()
                        .filter_map(|e| {
                            if let Event::Text(t) = e {
                                Some(t.as_ref())
                            } else {
                                None
                            }
                        })
                        .collect::<String>();

                    if let Some(original_dest) = &current_link_dest {
                        if !is_external && link_text.trim() == original_dest.trim() {
                            let dest_path = PathBuf::from(original_dest);
                            if let Some(auto_title) = get_link_title(
                                path_rel,
                                &dest_path,
                                metadata_map,
                                site_map,
                                args.verbose,
                            ) {
                                events.push(Event::Text(auto_title.into()));
                            } else {
                                events.append(&mut link_text_events);
                            }
                        } else {
                            events.append(&mut link_text_events);
                        }
                    } else {
                        events.append(&mut link_text_events);
                    }
                }

                in_link = false;
                current_link_dest = None;
                link_text_events.clear();
                should_auto_title = false;

                if is_external {
                    events.push(Event::Html("</a>".into()));
                } else {
                    events.push(Event::End(Tag::Link(link_type, dest, title_attr)));
                }
            }
            e => {
                if in_link {
                    link_text_events.push(e);
                } else {
                    events.push(e);
                }
            }
        }
    }

    let final_title = if !title_h1.is_empty() {
        title_h1
    } else {
        path_rel.to_string_lossy().to_string()
    };
    let html_from_events = events_to_html(events);
    let final_content = html_output + &html_from_events;
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

pub fn rewrite_link_to_relative(
    from_path_rel: &Path,
    link_target: &Path,
    site_map: &SiteMap,
    verbose: bool,
) -> String {
    let root_rel_path = resolve_link_path(from_path_rel, link_target);
    let target_path_rel = root_rel_path.strip_prefix("/").unwrap_or(Path::new(""));
    let mut final_target_path = target_path_rel.to_path_buf();

    if target_path_rel.extension().is_some_and(|ext| ext == "md") {
        final_target_path.set_extension("html");
    } else if target_path_rel.is_dir()
        || target_path_rel.extension().is_none()
        || target_path_rel.to_string_lossy().is_empty()
    {
        let target_is_index_md = target_path_rel.join("index.md");
        if target_path_rel.as_os_str().is_empty() || site_map.contains(&target_is_index_md) {
            final_target_path = target_path_rel.join("index.html");
        }
    }

    let current_dir = from_path_rel.parent().unwrap_or(Path::new(""));
    let rel_path =
        pathdiff::diff_paths(&final_target_path, current_dir).unwrap_or(final_target_path.clone());
    let rel_path_str = rel_path.to_string_lossy();

    if verbose {
        print_info(&format!(
            "Link rewrite: {} -> {} (via {})",
            link_target.display(),
            rel_path_str,
            from_path_rel.display()
        ));
    }
    rel_path_str.to_string()
}

fn get_link_title(
    from_path_rel: &Path,
    link_target: &Path,
    metadata_map: &MetadataMap,
    site_map: &SiteMap,
    verbose: bool,
) -> Option<String> {
    let root_rel_path = resolve_link_path(from_path_rel, link_target);
    let target_path_rel = root_rel_path.strip_prefix("/").unwrap_or(Path::new(""));

    let mut md_path = target_path_rel.to_path_buf();

    if md_path.extension().is_some_and(|ext| ext == "html") {
        md_path.set_extension("md");
    } else if md_path.is_dir()
        || md_path.extension().is_none()
        || target_path_rel.to_string_lossy().is_empty()
    {
        let index_path = md_path.join("index.md");
        if site_map.contains(&index_path) {
            md_path = index_path;
        } else {
            md_path = md_path.join("index.md");
        }
    }

    if verbose {
        print_info(&format!(
            "Looking up title for: {} (resolved to: {})",
            link_target.display(),
            md_path.display()
        ));
    }

    let result = metadata_map.get(&md_path).and_then(|meta| {
        meta.nav_title
            .clone()
            .or_else(|| meta.page_title.clone())
            .or_else(|| meta.computed_title.clone())
    });

    if verbose {
        if let Some(ref title) = result {
            print_info(&format!("  Found title: '{}'", title));
        } else {
            print_warning(&format!("  No title found for: {}", md_path.display()));
        }
    }

    result
}
