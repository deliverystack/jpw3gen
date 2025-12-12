use std::{fs}; 

mod config;
mod args;
mod io;
mod site_map;
mod nav;
mod processing;

use config::Args;
use args::parse_args;
use io::{read_template, print_info, print_error};
use site_map::build_site_map;
use processing::{process_directory, load_all_metadata_from_files}; // load_all_metadata_from_files is now correctly imported
use nav::generate_all_index_files;

fn main() -> std::io::Result<()> {
    let args: Args = parse_args();

    if args.verbose {
        print_info(&format!("Verbose mode enabled."));
        print_info(&format!("Source directory: {}", args.source.display()));
        print_info(&format!("Target directory: {}", args.target.display()));
    }

    if args.target.exists() && args.target.is_dir() {
        if args.verbose {
            print_info(&format!("Ensuring target directory structure exists: {}", args.target.display()));
        }
    } else {
        if args.verbose {
            print_info(&format!("Creating target directory: {}", args.target.display()));
        }
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
        print_info(&format!("Identified {} files for processing.", site_map.len()));
    }
    
    // NEW: Load all metadata before processing any files
    let metadata_map = load_all_metadata_from_files(&args, &site_map)?;

    // FIX: Pass the metadata_map to the processing and index generation functions
    process_directory(&args, &site_map, &metadata_map, &args.source, &html_template)?;
    
    generate_all_index_files(&args, &site_map, &metadata_map, &html_template)?;

    println!("Done processing directories.");
    Ok(())
}