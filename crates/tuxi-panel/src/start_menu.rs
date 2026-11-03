//! Descoberta e lançamento de aplicações via FreeDesktop / Gio.
//!
//! Destinado a extrair-se para o crate `tuxi-apps` quando estabilizar.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use relm4::gtk::gio::{prelude::*, AppInfo, AppLaunchContext};
use relm4::gtk::glib;
use tracing::{debug, warn};

/// Categorias FreeDesktop exibidas no menu (ordem estável).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum AppCategory {
    Development,
    Education,
    Graphics,
    Internet,
    Multimedia,
    Office,
    Science,
    Settings,
    System,
    Utility,
    Other,
}

impl AppCategory {
    pub(crate) const ALL: [AppCategory; 11] = [
        Self::Development,
        Self::Education,
        Self::Graphics,
        Self::Internet,
        Self::Multimedia,
        Self::Office,
        Self::Science,
        Self::Settings,
        Self::System,
        Self::Utility,
        Self::Other,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Development => "Desenvolvimento",
            Self::Education => "Educação",
            Self::Graphics => "Gráficos",
            Self::Internet => "Internet",
            Self::Multimedia => "Multimídia",
            Self::Office => "Escritório",
            Self::Science => "Ciência",
            Self::Settings => "Configurações",
            Self::System => "Sistema",
            Self::Utility => "Utilitários",
            Self::Other => "Outros",
        }
    }

    fn from_desktop_categories(categories: &str) -> Self {
        let has = |token: &str| categories.split(';').any(|c| c.eq_ignore_ascii_case(token));

        if has("Development") {
            Self::Development
        } else if has("Education") {
            Self::Education
        } else if has("Graphics") {
            Self::Graphics
        } else if has("Network") || has("WebBrowser") || has("Email") {
            Self::Internet
        } else if has("AudioVideo") || has("Audio") || has("Video") {
            Self::Multimedia
        } else if has("Office") {
            Self::Office
        } else if has("Science") {
            Self::Science
        } else if has("Settings") || has("DesktopSettings") {
            Self::Settings
        } else if has("System") {
            Self::System
        } else if has("Utility") {
            Self::Utility
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DesktopApp {
    pub id: String,
    pub name: String,
    info: AppInfo,
}

impl DesktopApp {
    pub(crate) fn launch(&self) -> Result<()> {
        let ctx = AppLaunchContext::new();
        self.info
            .launch(&[], Some(&ctx))
            .with_context(|| format!("falha ao iniciar {}", self.id))?;
        debug!(app = %self.id, "aplicação iniciada");
        Ok(())
    }
}

/// Lista aplicações visíveis, agrupadas por categoria FreeDesktop.
pub(crate) fn load_apps_by_category() -> BTreeMap<AppCategory, Vec<DesktopApp>> {
    let mut map: BTreeMap<AppCategory, Vec<DesktopApp>> = BTreeMap::new();

    for info in AppInfo::all() {
        if !info.should_show() {
            continue;
        }

        let name = info.display_name();
        if name.is_empty() {
            continue;
        }

        let id = info
            .id()
            .map(|s| s.to_string())
            .unwrap_or_else(|| name.to_string());

        let category = read_categories_from_desktop_file(&id)
            .map(|c| AppCategory::from_desktop_categories(&c))
            .unwrap_or(AppCategory::Other);

        map.entry(category).or_default().push(DesktopApp {
            id,
            name: name.to_string(),
            info,
        });
    }

    for apps in map.values_mut() {
        apps.sort_by_key(|a| a.name.to_lowercase());
    }

    map
}

/// Lê `Categories` do arquivo `.desktop` (Desktop Entry Spec / XDG).
///
/// O binding Rust do Gio 0.22 não expõe mais `DesktopAppInfo`; usamos `GKeyFile`
/// sobre os caminhos XDG documentados.
fn read_categories_from_desktop_file(id: &str) -> Option<String> {
    let path = resolve_desktop_file(id)?;
    let keyfile = glib::KeyFile::new();
    keyfile
        .load_from_file(&path, glib::KeyFileFlags::NONE)
        .ok()?;
    keyfile
        .string("Desktop Entry", "Categories")
        .ok()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

fn resolve_desktop_file(id: &str) -> Option<PathBuf> {
    let file_name = if id.ends_with(".desktop") {
        id.to_string()
    } else {
        format!("{id}.desktop")
    };

    let mut dirs: Vec<PathBuf> = Vec::new();
    dirs.push(glib::user_data_dir().join("applications"));
    for dir in glib::system_data_dirs() {
        dirs.push(dir.join("applications"));
    }

    for dir in dirs {
        let candidate = dir.join(&file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Filtra apps cujo nome contém `query` (case-insensitive).
pub(crate) fn filter_apps<'a>(
    apps: &'a BTreeMap<AppCategory, Vec<DesktopApp>>,
    query: &str,
) -> BTreeMap<AppCategory, Vec<&'a DesktopApp>> {
    let q = query.trim().to_lowercase();
    let mut out: BTreeMap<AppCategory, Vec<&DesktopApp>> = BTreeMap::new();

    for (cat, list) in apps {
        for app in list {
            if q.is_empty() || app.name.to_lowercase().contains(&q) {
                out.entry(*cat).or_default().push(app);
            }
        }
    }

    out
}

pub(crate) fn launch_by_id(apps: &BTreeMap<AppCategory, Vec<DesktopApp>>, id: &str) -> Result<()> {
    for list in apps.values() {
        if let Some(app) = list.iter().find(|a| a.id == id) {
            return app.launch();
        }
    }
    warn!(app = %id, "aplicação não encontrada no cache do menu");
    bail!("aplicação não encontrada: {id}")
}
