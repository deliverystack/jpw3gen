use std::{fs, io, path::Path, collections::HashSet};
use crate::config::SiteMap;

pub fn build_site_map(source_dir: &Path) -> io::Result<SiteMap> {
    let mut site_map = HashSet::new();
    
    fn traverse(dir: &Path, source_root: &Path, map: &mut SiteMap) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if name.starts_with('.') {
                        continue;
                    }
                }
                traverse(&path, source_root, map)?;
            } else if path.is_file() {
                if let Ok(rel_path) = path.strip_prefix(source_root) {
                    map.insert(rel_path.to_path_buf());
                }
            }
        }
        Ok(())
    }

    traverse(source_dir, source_dir, &mut site_map)?;
    Ok(site_map)
}