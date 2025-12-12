use std::path::PathBuf;
use clap::{Arg, Command};
use crate::config::Args;
use crate::io::print_error;

/// parse command line arguments
pub fn parse_args() -> Args {
    let matches = Command::new("Directory Copier")
        .version("1.0")
        .author("John West <github-public@c73.me>")
        .about("Create a static website from a directory structure containing markdown files.")
        .arg(
            Arg::new("source")
                .short('s') 
                .long("source")
                .value_parser(clap::value_parser!(String))
                .value_name("SOURCE_DIR")
                .help("Specifies the source directory (defaults to current directory if not provided)"),
        )
        .arg(
            Arg::new("target")
                .short('t') 
                .long("target")
                .value_parser(clap::value_parser!(String))
                .required(true)
                .value_name("TARGET_DIR")
                .help("Specifies the target directory where files will be copied"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(clap::ArgAction::SetTrue)
                .help("Enables verbose output"),
        )
        .get_matches();

    let source_dir_str = matches
        .get_one::<String>("source")
        .cloned()
        .unwrap_or_else(|| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| {
                    print_error("Unable to determine the current directory.");
                    std::process::exit(1);
                })
        });

    let target_dir_str = matches.get_one::<String>("target").unwrap();

    Args {
        source: PathBuf::from(source_dir_str),
        target: PathBuf::from(target_dir_str),
        verbose: *matches.get_one::<bool>("verbose").unwrap_or(&false),
    }
}