use std::{
    path::PathBuf,
    collections::{HashSet, BTreeMap},
};

#[derive(Debug, Clone)]
pub enum NavItem {
    File {
        // Full relative path from the source root, e.g., "docs/about.md"
        rel_path: PathBuf,
        // The display name (e.g., "about.html")
        name: String, 
        is_current: bool,
    },
    Directory {
        // Full relative path from the source root, e.g., "docs"
        rel_path: PathBuf,
        // The display name (e.g., "docs")
        name: String,
        // Map of children, keyed by name for sorting
        children: BTreeMap<String, NavItem>, 
    },
}

// Helper implementation for mutable tree traversal
impl NavItem {
    pub fn get_children_mut(&mut self) -> Option<&mut BTreeMap<String, NavItem>> {
        match self {
            NavItem::Directory { children, .. } => Some(children),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct Args {
    pub source: PathBuf,
    pub target: PathBuf,
    pub verbose: bool,
}

pub type NavTree = BTreeMap<String, NavItem>; 

/// A global map of all files to easily check for links.
pub type SiteMap = HashSet<PathBuf>;

pub const COLOR_RED: &str = "\x1b[31m";    
pub const COLOR_YELLOW: &str = "\x1b[33m"; 
pub const COLOR_CYAN: &str = "\x1b[36m";   
pub const COLOR_RESET: &str = "\x1b[0m";