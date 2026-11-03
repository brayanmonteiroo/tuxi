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

    /// Identificador estável para guardar na row GTK (`widget_name`).
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Education => "education",
            Self::Graphics => "graphics",
            Self::Internet => "internet",
            Self::Multimedia => "multimedia",
            Self::Office => "office",
            Self::Science => "science",
            Self::Settings => "settings",
            Self::System => "system",
            Self::Utility => "utility",
            Self::Other => "other",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.id() == id)
    }

    pub(crate) fn from_desktop_categories(categories: &str) -> Self {
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
    let _span = tracing::info_span!("load_apps_by_category").entered();
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

    let total: usize = map.values().map(Vec::len).sum();
    debug!(categories = map.len(), apps = total, "apps carregadas");
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

/// `true` se o nome passa o filtro de pesquisa (case-insensitive; vazio = tudo).
pub(crate) fn matches_search_query(name: &str, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || name.to_lowercase().contains(&q)
}

/// Filtra apps cujo nome contém `query` (case-insensitive).
pub(crate) fn filter_apps<'a>(
    apps: &'a BTreeMap<AppCategory, Vec<DesktopApp>>,
    query: &str,
) -> BTreeMap<AppCategory, Vec<&'a DesktopApp>> {
    let _span = tracing::debug_span!("filter_apps", query = query).entered();
    let mut out: BTreeMap<AppCategory, Vec<&DesktopApp>> = BTreeMap::new();

    for (cat, list) in apps {
        for app in list {
            if matches_search_query(&app.name, query) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_id_label_roundtrip() {
        for cat in AppCategory::ALL {
            assert_eq!(AppCategory::from_id(cat.id()), Some(cat));
            assert!(!cat.label().is_empty());
            assert!(!cat.id().is_empty());
        }
        assert_eq!(AppCategory::from_id("nope"), None);
    }

    #[test]
    fn from_desktop_categories_mapping() {
        assert_eq!(
            AppCategory::from_desktop_categories("Development;IDE;"),
            AppCategory::Development
        );
        assert_eq!(
            AppCategory::from_desktop_categories("Network;WebBrowser;"),
            AppCategory::Internet
        );
        assert_eq!(
            AppCategory::from_desktop_categories("AudioVideo;Player;"),
            AppCategory::Multimedia
        );
        assert_eq!(
            AppCategory::from_desktop_categories("Settings;DesktopSettings;"),
            AppCategory::Settings
        );
        assert_eq!(
            AppCategory::from_desktop_categories("Game;"),
            AppCategory::Other
        );
        assert_eq!(
            AppCategory::from_desktop_categories("development;"),
            AppCategory::Development
        );
    }

    #[test]
    fn matches_search_query_empty_and_case() {
        assert!(matches_search_query("Firefox", ""));
        assert!(matches_search_query("Firefox", "   "));
        assert!(matches_search_query("Firefox", "fire"));
        assert!(matches_search_query("Firefox", "FIRE"));
        assert!(!matches_search_query("Firefox", "chrome"));
    }

    #[test]
    fn filter_apps_by_query_on_names() {
        // Sem AppInfo: exercita matches_search_query como o filtro real usa.
        let names = ["Android Studio", "Firefox", "Kate"];
        let hit_fire: Vec<_> = names
            .iter()
            .copied()
            .filter(|n| matches_search_query(n, "fire"))
            .collect();
        assert_eq!(hit_fire, vec!["Firefox"]);

        let hit_a: Vec<_> = names
            .iter()
            .copied()
            .filter(|n| matches_search_query(n, "a"))
            .collect();
        assert_eq!(hit_a, vec!["Android Studio", "Kate"]);

        let all: Vec<_> = names
            .iter()
            .copied()
            .filter(|n| matches_search_query(n, ""))
            .collect();
        assert_eq!(all.len(), 3);
    }
}
