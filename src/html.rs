use regex::Regex;
use std::{fs, io, path::Path};

use crate::config::{Args, MetadataMap};
use crate::io::{print_info, print_warning};
use crate::processing::get_last_modified_date;

pub fn format_html_page(
    title: &str,
    rel_path_str: &str,
    date_created: &str,
    last_modified_time: &str,
    nav_html: &str,
    content: &str,
    html_template: &str,
    breadcrumb_html: &str,
    canonical_url: &str,
) -> String {
    html_template
        .replace("{{ title }}", title)
        .replace("{{ header_title }}", title)
        .replace("{{ source_path }}", rel_path_str)
        .replace("{{ date_created }}", date_created)
        .replace("{{ last_modified }}", last_modified_time)
        .replace("{{ nav_html }}", nav_html)
        .replace("{{ content }}", content)
        .replace("{{ breadcrumb_html }}", breadcrumb_html)
        .replace("{{ canonical_url }}", canonical_url)
}

pub fn generate_breadcrumb_html(
    rel_path: &Path,
    metadata_map: &MetadataMap,
    _base_url: &str,
) -> String {
    let mut breadcrumbs = Vec::new();
    let mut current_path = std::path::PathBuf::new();

    // Add home link
    breadcrumbs.push(format!(r#"<a href="/">Home</a>"#));

    // Get path components
    let components: Vec<_> = rel_path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(os_str) => Some(os_str.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();

    if components.is_empty() {
        return r#"<nav class="breadcrumb"><a href="/">Home</a></nav>"#.to_string();
    }

    // Build breadcrumb for each component
    for (i, component) in components.iter().enumerate() {
        current_path.push(component);

        let is_last = i == components.len() - 1;

        if is_last && rel_path.file_name().is_some() {
            // Last component is a file
            let display_name = if let Some(stem) = rel_path.file_stem() {
                let stem_str = stem.to_string_lossy();
                if stem_str == "index" {
                    // For index files, use the parent directory name or metadata
                    let index_path = current_path.with_extension("md");
                    metadata_map
                        .get(&index_path)
                        .and_then(|m| m.nav_title.clone().or_else(|| m.computed_title.clone()))
                        .or_else(|| {
                            current_path
                                .parent()
                                .and_then(|p| p.file_name())
                                .map(|n| n.to_string_lossy().to_string())
                        })
                        .unwrap_or_else(|| stem_str.to_string())
                } else {
                    // For regular files, try nav_title first, then computed_title, then filename
                    let file_path = current_path.with_extension("md");
                    metadata_map
                        .get(&file_path)
                        .and_then(|m| m.nav_title.clone().or_else(|| m.computed_title.clone()))
                        .unwrap_or_else(|| stem_str.to_string())
                }
            } else {
                component.clone()
            };

            breadcrumbs.push(format!(
                r#"<span class="breadcrumb-current">{}</span>"#,
                display_name
            ));
        } else {
            // Directory component
            let index_path = current_path.join("index.md");
            let display_name = metadata_map
                .get(&index_path)
                .and_then(|m| m.nav_title.clone().or_else(|| m.computed_title.clone()))
                .unwrap_or_else(|| component.clone());

            let href = if i == components.len() - 1 {
                format!("/{}/", current_path.to_string_lossy())
            } else {
                format!("/{}/", current_path.to_string_lossy())
            };

            breadcrumbs.push(format!(r#"<a href="{}">{}</a>"#, href, display_name));
        }
    }

    format!(
        r#"<nav class="breadcrumb">{}</nav>"#,
        breadcrumbs.join(" › ")
    )
}

pub fn generate_canonical_url(rel_path: &Path, base_url: &str) -> String {
    let mut url_path = rel_path.to_path_buf();

    // Convert index.md to directory path
    if rel_path.file_name().is_some_and(|n| n == "index.md") {
        if rel_path.parent().is_some_and(|p| p.as_os_str().is_empty()) {
            // Root index
            return format!("{}/", base_url.trim_end_matches('/'));
        } else {
            url_path = rel_path.parent().unwrap().to_path_buf();
            return format!(
                "{}/{}/",
                base_url.trim_end_matches('/'),
                url_path.to_string_lossy()
            );
        }
    }

    // Convert .md to .html
    url_path.set_extension("html");

    let path_str = url_path.to_string_lossy();
    if path_str.is_empty() {
        format!("{}/", base_url.trim_end_matches('/'))
    } else {
        format!("{}/{}", base_url.trim_end_matches('/'), path_str)
    }
}

pub fn convert_urls_to_anchors(html: &str) -> String {
    let url_regex = Regex::new(r"https?://[^\s<]+").unwrap();
    let anchor_regex = Regex::new(r"<a\b[^>]*>.*?</a>").unwrap();

    let mut result = String::new();
    let mut last_pos = 0;

    let mut anchor_ranges = Vec::new();
    for mat in anchor_regex.find_iter(html) {
        anchor_ranges.push((mat.start(), mat.end()));
    }

    for url_match in url_regex.find_iter(html) {
        let start = url_match.start();
        let end = url_match.end();

        let in_anchor = anchor_ranges
            .iter()
            .any(|(a_start, a_end)| start >= *a_start && end <= *a_end);

        if !in_anchor {
            result.push_str(&html[last_pos..start]);

            let url = url_match.as_str();
            let is_external = url.starts_with("http://") || url.starts_with("https://");
            if is_external {
                result.push_str(&format!(
                    "<a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">{}</a>",
                    url, url
                ));
            } else {
                result.push_str(&format!("<a href=\"{}\">{}</a>", url, url));
            }

            last_pos = end;
        }
    }

    result.push_str(&html[last_pos..]);

    if result.is_empty() {
        html.to_string()
    } else {
        result
    }
}

pub fn generate_sitemap_xml(args: &Args, metadata_map: &MetadataMap) -> io::Result<()> {
    let sitemap_path = args.target.join("sitemap.xml");

    let default_changefreq = "monthly";
    let base_priority = 0.5;

    let mut entries = Vec::new();

    for (rel_path, metadata) in metadata_map.iter() {
        if metadata.include_in_sitemap.unwrap_or(false) {
            let mut url_path = rel_path.to_path_buf();

            if rel_path.file_name().is_some_and(|n| n == "index.md") {
                if rel_path.parent().is_some_and(|p| p.as_os_str().is_empty()) {
                    url_path = std::path::PathBuf::from("");
                } else {
                    url_path = rel_path.parent().unwrap().to_path_buf();
                }
            } else {
                url_path.set_extension("html");
            }

            let loc_url = {
                let path_str = url_path.to_string_lossy();
                if path_str.is_empty() {
                    "/".to_string()
                } else {
                    format!("/{}", path_str)
                }
            };

            let source_path = args.source.join(rel_path);
            let last_mod = get_last_modified_date(&source_path);

            let change_freq = metadata
                .sitemap_changefreq
                .as_deref()
                .unwrap_or(default_changefreq);

            let priority_float = metadata
                .sitemap_priority
                .unwrap_or(base_priority)
                .clamp(0.0, 1.0);

            let priority = format!("{:.1}", priority_float);

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

    fs::write(&sitemap_path, &xml_content)?;

    if args.verbose {
        print_info(&format!(
            "Successfully generated sitemap.xml at: {}",
            sitemap_path.display()
        ));
    }

    Ok(())
}
