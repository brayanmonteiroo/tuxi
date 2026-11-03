//! Pastas XDG (Desktop / Templates) e config TUXI.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub(crate) fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub(crate) fn config_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("tuxi")
    } else {
        home_dir().join(".config/tuxi")
    }
}

pub(crate) fn layout_path() -> PathBuf {
    config_dir().join("desktop-layout.json")
}

/// Resolve `XDG_DESKTOP_DIR` (ex.: ~/Área de trabalho).
pub(crate) fn desktop_dir() -> PathBuf {
    read_user_dir("XDG_DESKTOP_DIR").unwrap_or_else(|| home_dir().join("Desktop"))
}

pub(crate) fn templates_dir() -> PathBuf {
    read_user_dir("XDG_TEMPLATES_DIR").unwrap_or_else(|| home_dir().join("Templates"))
}

fn read_user_dir(key: &str) -> Option<PathBuf> {
    if let Ok(val) = std::env::var(key) {
        if !val.is_empty() {
            return Some(expand_home(&val));
        }
    }
    let dirs_file = config_home().join("user-dirs.dirs");
    let Ok(text) = fs::read_to_string(dirs_file) else {
        return None;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() != key {
            continue;
        }
        let v = v.trim().trim_matches('"');
        return Some(expand_home(v));
    }
    None
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"))
}

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("$HOME/") {
        home_dir().join(rest)
    } else if path == "$HOME" {
        home_dir()
    } else {
        PathBuf::from(path)
    }
}

pub(crate) fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("criar {}", path.display()))
}

/// Templates: pasta do user + `packaging/templates` (dev) + fallbacks embutidos.
pub(crate) fn list_template_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for dir in template_search_dirs() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if name.starts_with('.') {
                continue;
            }
            if seen.insert(name) {
                out.push(path);
            }
        }
    }
    out.sort_by(|a, b| {
        a.file_name()
            .unwrap_or_default()
            .cmp(b.file_name().unwrap_or_default())
    });
    out
}

fn template_search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![templates_dir()];
    // Dev: packaging/templates junto ao crate / repo.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dirs.push(manifest.join("../../packaging/templates"));
    dirs.push(PathBuf::from("/usr/local/share/tuxi/templates"));
    dirs.push(PathBuf::from("/usr/share/templates"));
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_home_path() {
        let p = expand_home("$HOME/Área de trabalho");
        assert!(p.ends_with("Área de trabalho"));
    }
}
