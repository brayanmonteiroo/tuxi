//! Persistência de posições e wallpaper.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::xdg;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct DesktopLayout {
    #[serde(default)]
    pub wallpaper: Option<PathBuf>,
    #[serde(default)]
    pub icons: HashMap<String, IconPos>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub(crate) struct IconPos {
    pub x: f64,
    pub y: f64,
}

impl DesktopLayout {
    pub(crate) fn load() -> Self {
        let path = xdg::layout_path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub(crate) fn save(&self) -> Result<()> {
        xdg::ensure_dir(&xdg::config_dir())?;
        let path = xdg::layout_path();
        let text = serde_json::to_string_pretty(self).context("serializar layout")?;
        fs::write(&path, text).with_context(|| format!("gravar {}", path.display()))?;
        Ok(())
    }

    pub(crate) fn set_icon(&mut self, name: &str, x: f64, y: f64) {
        self.icons.insert(name.to_string(), IconPos { x, y });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_json() {
        let mut l = DesktopLayout::default();
        l.set_icon("a.txt", 10.0, 20.0);
        let s = serde_json::to_string(&l).unwrap();
        let back: DesktopLayout = serde_json::from_str(&s).unwrap();
        assert_eq!(back.icons["a.txt"].x, 10.0);
    }
}
