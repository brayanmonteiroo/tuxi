//! Descoberta e lançamento de aplicações via FreeDesktop / Gio.
//!
//! Destinado a extrair-se para o crate `tuxi-apps` quando estabilizar.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context, Result};
use relm4::gtk::gdk::{self, prelude::DisplayExt};
use relm4::gtk::gio::{self, prelude::*, AppInfo};
use relm4::gtk::glib;
use tracing::{debug, warn};

use crate::session_power::is_nested_session;

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

    /// Ícone simbólico FreeDesktop (tema do sistema: Adwaita/Papirus/…).
    pub(crate) fn icon_name(self) -> &'static str {
        match self {
            Self::Development => "applications-development-symbolic",
            Self::Education => "applications-science-symbolic",
            Self::Graphics => "applications-graphics-symbolic",
            Self::Internet => "applications-internet-symbolic",
            Self::Multimedia => "applications-multimedia-symbolic",
            Self::Office => "applications-office-symbolic",
            Self::Science => "applications-science-symbolic",
            Self::Settings => "preferences-system-symbolic",
            Self::System => "applications-system-symbolic",
            Self::Utility => "applications-utilities-symbolic",
            Self::Other => "applications-other-symbolic",
        }
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
        if is_nested_session() {
            // Gio/D-BusActivatable costuma ativar no *host* (Plasma). No nested
            // spawnamos o Exec com o WAYLAND_DISPLAY do labwc.
            return launch_nested_spawn(self);
        }

        let display = gdk::Display::default().context("nenhum display GDK")?;
        let ctx = display.app_launch_context();
        if let Ok(wd) = std::env::var("WAYLAND_DISPLAY") {
            gio::prelude::AppLaunchContextExt::setenv(&ctx, "WAYLAND_DISPLAY", &wd);
        }

        self.info
            .launch(&[], Some(&ctx))
            .with_context(|| format!("falha ao iniciar {}", self.id))?;
        debug!(app = %self.id, "aplicação iniciada");
        Ok(())
    }

    /// Ícone FreeDesktop do `.desktop`, se o tema o resolver.
    pub(crate) fn icon(&self) -> Option<relm4::gtk::gio::Icon> {
        self.info.icon()
    }
}

/// Arranca a app como filho do painel no compositor nested.
///
/// Electron/Flatpak/D-BusActivatable tendem a abrir no *host* se usarem o
/// session bus ou o DISPLAY do Plasma — por isso forçamos Wayland do nested
/// e removemos DISPLAY.
fn launch_nested_spawn(app: &DesktopApp) -> Result<()> {
    let wd = std::env::var("WAYLAND_DISPLAY")
        .context("WAYLAND_DISPLAY em falta no nested")?;

    let raw = app
        .info
        .commandline()
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            let exe = app.info.executable();
            if exe.as_os_str().is_empty() {
                None
            } else {
                Some(exe.to_string_lossy().into_owned())
            }
        })
        .with_context(|| format!("sem Exec/commandline para {}", app.id))?;

    let cmd = strip_desktop_field_codes(&raw);
    if cmd.trim().is_empty() {
        bail!("Exec vazio após limpar field codes: {}", app.id);
    }
    let cmd = wrap_flatpak_wayland(&cmd, &wd);
    let cmd = ensure_electron_basic_password_store(&cmd);

    debug!(app = %app.id, %cmd, wayland = %wd, "spawn no nested");
    Command::new("/bin/sh")
        .arg("-c")
        .arg(&cmd)
        .env("WAYLAND_DISPLAY", &wd)
        .env("GDK_BACKEND", "wayland")
        .env("QT_QPA_PLATFORM", "wayland")
        .env("ELECTRON_OZONE_PLATFORM_HINT", "wayland")
        .env("SDL_VIDEODRIVER", "wayland")
        .env("XDG_CURRENT_DESKTOP", "TUXI:labwc")
        .env("XDG_SESSION_DESKTOP", "tuxi")
        // Evita Qt/Electron puxarem KWallet / tema KDE do host.
        .env("PASSWORD_STORE", "basic")
        .env("CHROME_PASSWORD_STORE", "basic")
        .env("GTK_CSD", "0")
        .env_remove("DISPLAY")
        .env_remove("KDE_FULL_SESSION")
        .env_remove("KDE_SESSION_VERSION")
        .env_remove("QT_QPA_PLATFORMTHEME")
        .env_remove("DESKTOP_SESSION")
        .env_remove("GNOME_KEYRING_CONTROL")
        .env_remove("GNOME_KEYRING_PID")
        .spawn()
        .with_context(|| format!("falha ao spawnar {}", app.id))?;
    Ok(())
}

/// Electron/AppImage: força store básico (sem KWallet) na CLI.
fn ensure_electron_basic_password_store(cmd: &str) -> String {
    let lower = cmd.to_ascii_lowercase();
    if lower.contains("--password-store=") {
        return cmd.to_string();
    }
    if lower.contains(".appimage")
        || lower.contains("electron")
        || lower.contains("chrom")
    {
        format!("{cmd} --password-store=basic")
    } else {
        cmd.to_string()
    }
}

/// Flatpak limpa env — injeta `--env=` após `flatpak run`.
fn wrap_flatpak_wayland(cmd: &str, wd: &str) -> String {
    let trimmed = cmd.trim_start();
    let Some(rest) = trimmed
        .strip_prefix("flatpak run ")
        .or_else(|| trimmed.strip_prefix("/usr/bin/flatpak run "))
    else {
        return cmd.to_string();
    };
    format!(
        "flatpak run --env=WAYLAND_DISPLAY={wd} --env=GDK_BACKEND=wayland \
         --env=QT_QPA_PLATFORM=wayland --env=ELECTRON_OZONE_PLATFORM_HINT=wayland \
         --env=PASSWORD_STORE=basic --env=CHROME_PASSWORD_STORE=basic \
         --env=XDG_CURRENT_DESKTOP=TUXI:labwc {rest}"
    )
}

/// Campo `Icon=` do `.desktop` que casa com app_id / título / WMClass.
pub(crate) fn lookup_desktop_icon_field(needles: &[String]) -> Option<String> {
    if needles.is_empty() {
        return None;
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    dirs.push(glib::user_data_dir().join("applications"));
    for dir in glib::system_data_dirs() {
        dirs.push(dir.join("applications"));
    }

    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let keyfile = glib::KeyFile::new();
            if keyfile
                .load_from_file(&path, glib::KeyFileFlags::NONE)
                .is_err()
            {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            let name = keyfile
                .string("Desktop Entry", "Name")
                .ok()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let wm = keyfile
                .string("Desktop Entry", "StartupWMClass")
                .ok()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let hits = needles.iter().any(|n| {
                stem.eq_ignore_ascii_case(n)
                    || name.eq_ignore_ascii_case(n)
                    || (!wm.is_empty() && wm.eq_ignore_ascii_case(n))
            });
            if !hits {
                continue;
            }
            if let Ok(icon) = keyfile.string("Desktop Entry", "Icon") {
                let icon = icon.to_string();
                if !icon.is_empty() {
                    return Some(icon);
                }
            }
        }
    }
    None
}

/// Remove `%u`, `%f`, `%%`, etc. do Exec (Desktop Entry Spec).
fn strip_desktop_field_codes(exec: &str) -> String {
    let mut out = String::with_capacity(exec.len());
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some('%') => out.push('%'),
                Some(_) => {}
                None => out.push('%'),
            }
        } else {
            out.push(c);
        }
    }
    out
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
            assert!(
                cat.icon_name().ends_with("-symbolic"),
                "{} sem ícone simbólico",
                cat.id()
            );
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
    fn strip_desktop_field_codes_basic() {
        assert_eq!(
            strip_desktop_field_codes("firefox %u"),
            "firefox "
        );
        assert_eq!(strip_desktop_field_codes("foo %% bar"), "foo % bar");
        assert_eq!(strip_desktop_field_codes("gimp-3.0"), "gimp-3.0");
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
