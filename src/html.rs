use regex::Regex;
use std::{fs, io};

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
) -> String {
    html_template
        .replace("{{ title }}", title)
        .replace("{{ header_title }}", title)
        .replace("{{ source_path }}", rel_path_str)
        .replace("{{ date_created }}", date_created)
        .replace("{{ last_modified }}", last_modified_time)
        .replace("{{ nav_html }}", nav_html)
        .replace("{{ content }}", content)
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
