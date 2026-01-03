use std::fs;

mod args;
mod config;
mod html;
mod io;
mod markdown;
mod nav;
mod processing;
mod site_map;

use args::parse_args;
use config::Args;
use html::generate_sitemap_xml;
use io::{print_error, print_info, read_template};
use nav::generate_all_index_files;
use processing::{load_all_metadata_from_files, process_directory};
use site_map::build_site_map;

fn main() -> std::io::Result<()> {
    let args: Args = parse_args();

    if args.verbose {
        print_info("Verbose mode enabled.");
        print_info(&format!("Source directory: {}", args.source.display()));
        print_info(&format!("Target directory: {}", args.target.display()));
    }

    if args.target.exists() && args.target.is_dir() {
        if args.verbose {
            print_info(&format!(
                "Ensuring target directory structure exists: {}",
                args.target.display()
            ));
        }
    } else if args.verbose {
        print_info(&format!(
            "Creating target directory: {}",
            args.target.display()
        ));
    }

    fs::create_dir_all(&args.target)?;

    let html_template = match read_template(&args.source, &args) {
        Ok(template) => template,
        Err(e) => {
            print_error(&format!("Template Error: {}", e));
            return Err(e);
        }
    };

    let site_map = build_site_map(&args.source)?;
    if args.verbose {
        print_info(&format!(
            "Identified {} files for processing.",
            site_map.len()
        ));
    }

    let metadata_map = load_all_metadata_from_files(&args, &site_map)?;

    if args.verbose {
        print_info(&format!(
            "Loaded metadata for {} files.",
            metadata_map.len()
        ));
    }

    process_directory(
        &args,
        &site_map,
        &metadata_map,
        &args.source,
        &html_template,
    )?;

    generate_all_index_files(&args, &site_map, &metadata_map, &html_template)?;

    generate_sitemap_xml(&args, &metadata_map)?;

    println!("Done processing directories.");
    Ok(())
}
