//! Lista entradas da pasta Desktop.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

pub(crate) fn list_desktop_entries(dir: &Path) -> Result<Vec<DesktopEntry>> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for entry in fs::read_dir(dir).with_context(|| format!("ler {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let is_dir = path.is_dir();
        out.push(DesktopEntry { name, path, is_dir });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

pub(crate) fn fingerprint(dir: &Path) -> String {
    let Ok(rd) = fs::read_dir(dir) else {
        return String::new();
    };
    let mut parts: Vec<String> = rd
        .flatten()
        .map(|e| {
            let meta = e.metadata().ok();
            let mtime = meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("{}:{mtime}", e.file_name().to_string_lossy())
        })
        .collect();
    parts.sort();
    parts.join("|")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn lists_non_dotfiles() {
        let dir = std::env::temp_dir().join("tuxi-desktop-scan-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"x").unwrap();
        fs::write(dir.join(".hidden"), b"x").unwrap();
        let list = list_desktop_entries(&dir).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "a.txt");
        let _ = fs::remove_dir_all(&dir);
    }
}
