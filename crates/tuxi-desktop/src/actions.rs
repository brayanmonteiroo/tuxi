//! Acções do menu de contexto e abertura de ficheiros.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use tracing::{info, warn};

use crate::xdg;

pub(crate) fn open_path(path: &Path) -> Result<()> {
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .with_context(|| format!("xdg-open {}", path.display()))?;
    Ok(())
}

pub(crate) fn open_terminal_at(dir: &Path) -> Result<()> {
    let dir = dir.to_path_buf();
    let candidates = [
        std::env::var("TERMINAL").ok(),
        std::env::var("XDG_TERMINAL_EXEC").ok(),
        Some("xdg-terminal-exec".into()),
        Some("konsole".into()),
        Some("gnome-terminal".into()),
        Some("kitty".into()),
        Some("alacritty".into()),
        Some("ptyxis".into()),
    ];
    for cand in candidates.into_iter().flatten() {
        if cand.is_empty() {
            continue;
        }
        // xdg-terminal-exec / konsole --workdir
        let mut cmd = Command::new(&cand);
        match cand.as_str() {
            "konsole" => {
                cmd.arg("--workdir").arg(&dir);
            }
            "gnome-terminal" | "ptyxis" => {
                cmd.arg("--working-directory").arg(&dir);
            }
            "kitty" | "alacritty" => {
                cmd.arg("--working-directory").arg(&dir);
            }
            "xdg-terminal-exec" => {
                cmd.current_dir(&dir);
            }
            _ => {
                cmd.current_dir(&dir);
            }
        }
        match cmd.spawn() {
            Ok(_) => {
                info!(terminal = %cand, "terminal aberto");
                return Ok(());
            }
            Err(err) => warn!(terminal = %cand, ?err, "falha ao abrir terminal"),
        }
    }
    bail!("nenhum terminal encontrado (defina $TERMINAL)");
}

pub(crate) fn lock_session() -> Result<()> {
    let status = Command::new("loginctl")
        .arg("lock-session")
        .status()
        .context("loginctl lock-session")?;
    if !status.success() {
        bail!("loginctl saiu com {status}");
    }
    Ok(())
}

pub(crate) fn open_display_settings() -> Result<()> {
    let bin = find_settings_bin().context("tuxi-settings não encontrado")?;
    Command::new(bin)
        .args(["--page", "displays"])
        .spawn()
        .context("spawn tuxi-settings")?;
    Ok(())
}

fn find_settings_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("TUXI_SETTINGS") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    let home = xdg::home_dir();
    for c in [
        home.join("dev/github/tuxi/target/debug/tuxi-settings"),
        home.join("tuxi/target/debug/tuxi-settings"),
        PathBuf::from("/usr/local/bin/tuxi-settings"),
        PathBuf::from("tuxi-settings"),
    ] {
        if c.file_name().is_some() && (c.is_file() || c == PathBuf::from("tuxi-settings")) {
            if c == PathBuf::from("tuxi-settings") {
                return Some(c);
            }
            if c.is_file() {
                return Some(c);
            }
        }
    }
    which("tuxi-settings")
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let c = dir.join(name);
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

/// Nome sugerido no diálogo (já com ` (2)` se existir).
pub(crate) fn suggested_unique_name(desktop: &Path, base: &str) -> String {
    unique_path(desktop, base)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(base)
        .to_string()
}

pub(crate) fn create_folder(desktop: &Path, name: &str) -> Result<PathBuf> {
    let name = sanitize_name(name, "Nova pasta");
    let path = unique_path(desktop, &name);
    fs::create_dir(&path).with_context(|| format!("criar pasta {}", path.display()))?;
    Ok(path)
}

pub(crate) fn create_empty_file(desktop: &Path, name: &str) -> Result<PathBuf> {
    let name = sanitize_name(name, "Arquivo vazio.txt");
    let path = unique_path(desktop, &name);
    fs::write(&path, "").with_context(|| format!("criar {}", path.display()))?;
    Ok(path)
}

pub(crate) fn create_from_template(
    desktop: &Path,
    template: &Path,
    name: &str,
) -> Result<PathBuf> {
    let fallback = template
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Novo ficheiro");
    let name = sanitize_name(name, fallback);
    let dest = unique_path(desktop, &name);
    fs::copy(template, &dest)
        .with_context(|| format!("copiar template → {}", dest.display()))?;
    Ok(dest)
}

fn sanitize_name(name: &str, fallback: &str) -> String {
    let t = name.trim();
    if t.is_empty() || t.contains('/') || t.contains('\0') {
        fallback.to_string()
    } else {
        t.to_string()
    }
}

fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = split_name(name);
    for i in 2..1000 {
        let n = if ext.is_empty() {
            format!("{stem} ({i})")
        } else {
            format!("{stem} ({i}).{ext}")
        };
        let p = dir.join(n);
        if !p.exists() {
            return p;
        }
    }
    dir.join(format!("{name}.new"))
}

fn split_name(name: &str) -> (String, String) {
    if let Some((s, e)) = name.rsplit_once('.') {
        if !s.is_empty() {
            return (s.to_string(), e.to_string());
        }
    }
    (name.to_string(), String::new())
}

#[allow(dead_code)] // reservado para Colar (fora do menu por agora)
pub(crate) fn copy_paths_into_desktop(desktop: &Path, paths: &[PathBuf]) -> Result<usize> {
    let mut n = 0;
    for src in paths {
        if !src.exists() {
            continue;
        }
        let name = src
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("colado");
        let dest = unique_path(desktop, name);
        if src.is_dir() {
            copy_dir_recursive(src, &dest)?;
        } else {
            fs::copy(src, &dest)?;
        }
        n += 1;
    }
    Ok(n)
}

#[allow(dead_code)]
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
