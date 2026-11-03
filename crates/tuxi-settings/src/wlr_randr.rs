//! Bridge CLI para `wlr-randr` (labwc / wlroots).

use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OutputMode {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: f64,
    pub preferred: bool,
    pub current: bool,
}

impl OutputMode {
    pub(crate) fn label(&self) -> String {
        format!(
            "{}×{} @ {:.2} Hz{}",
            self.width,
            self.height,
            self.refresh_hz,
            if self.preferred { " ★" } else { "" }
        )
    }

    pub(crate) fn cli_mode(&self) -> String {
        format!("{}x{}@{:.6}", self.width, self.height, self.refresh_hz)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OutputHead {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub modes: Vec<OutputMode>,
    pub pos_x: i32,
    pub pos_y: i32,
    pub scale: f64,
    pub transform: String,
}

impl OutputHead {
    pub(crate) fn current_mode(&self) -> Option<&OutputMode> {
        self.modes.iter().find(|m| m.current).or_else(|| {
            self.modes.iter().find(|m| m.preferred)
        })
    }

    pub(crate) fn display_title(&self) -> String {
        let desc = self.description.trim().trim_matches('"');
        if desc.is_empty() || desc == "(null) (null)" {
            self.name.clone()
        } else {
            format!("{} — {}", self.name, desc)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ProbeResult {
    Ok(Vec<OutputHead>),
    MissingBinary,
    Error(String),
}

pub(crate) fn binary_path() -> Option<PathBuf> {
    which("wlr-randr")
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub(crate) fn probe_outputs() -> ProbeResult {
    let Some(bin) = binary_path() else {
        return ProbeResult::MissingBinary;
    };
    match Command::new(&bin).output() {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match parse_wlr_randr_list(&stdout) {
                Ok(heads) => ProbeResult::Ok(heads),
                Err(err) => ProbeResult::Error(err.to_string()),
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            ProbeResult::Error(if stderr.trim().is_empty() {
                format!("wlr-randr saiu com {}", out.status)
            } else {
                stderr.trim().to_string()
            })
        }
        Err(err) => ProbeResult::Error(err.to_string()),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ApplyRequest {
    pub name: String,
    pub enabled: bool,
    pub mode: Option<OutputMode>,
    pub pos_x: i32,
    pub pos_y: i32,
    pub scale: f64,
}

pub(crate) fn apply_output(req: &ApplyRequest) -> Result<()> {
    let bin = binary_path().context("wlr-randr não encontrado no PATH")?;
    let mut cmd = Command::new(&bin);
    cmd.arg("--output").arg(&req.name);
    if !req.enabled {
        cmd.arg("--off");
    } else {
        cmd.arg("--on");
        if let Some(mode) = &req.mode {
            cmd.arg("--mode").arg(mode.cli_mode());
        }
        cmd.arg("--pos")
            .arg(format!("{},{}", req.pos_x, req.pos_y));
        cmd.arg("--scale").arg(format!("{:.3}", req.scale));
    }
    let out = cmd
        .output()
        .with_context(|| format!("falha ao executar {}", bin.display()))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!(
            "wlr-randr falhou: {}",
            if stderr.trim().is_empty() {
                out.status.to_string()
            } else {
                stderr.trim().to_string()
            }
        );
    }
    Ok(())
}

/// Parseia a saída de `wlr-randr` sem argumentos.
pub(crate) fn parse_wlr_randr_list(text: &str) -> Result<Vec<OutputHead>> {
    let mut heads = Vec::new();
    let mut current: Option<OutputHead> = None;
    let mut in_modes = false;

    for raw_line in text.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }

        // Nova saída: `NAME "description"`
        if !line.starts_with(' ') && !line.starts_with('\t') {
            if let Some(head) = current.take() {
                heads.push(head);
            }
            in_modes = false;
            let (name, description) = split_output_header(line)?;
            current = Some(OutputHead {
                name,
                description,
                enabled: true,
                modes: Vec::new(),
                pos_x: 0,
                pos_y: 0,
                scale: 1.0,
                transform: "normal".into(),
            });
            continue;
        }

        let Some(head) = current.as_mut() else {
            continue;
        };
        let trimmed = line.trim();

        if trimmed == "Modes:" {
            in_modes = true;
            continue;
        }

        if in_modes {
            if let Some(mode) = parse_mode_line(trimmed) {
                head.modes.push(mode);
                continue;
            }
            // Saiu da secção Modes
            in_modes = false;
        }

        if let Some(v) = trimmed.strip_prefix("Enabled:") {
            head.enabled = v.trim().eq_ignore_ascii_case("yes");
        } else if let Some(v) = trimmed.strip_prefix("Position:") {
            let (x, y) = parse_pos(v.trim())?;
            head.pos_x = x;
            head.pos_y = y;
        } else if let Some(v) = trimmed.strip_prefix("Scale:") {
            head.scale = v.trim().parse().unwrap_or(1.0);
        } else if let Some(v) = trimmed.strip_prefix("Transform:") {
            head.transform = v.trim().to_string();
        }
    }

    if let Some(head) = current {
        heads.push(head);
    }
    Ok(heads)
}

fn split_output_header(line: &str) -> Result<(String, String)> {
    let line = line.trim();
    if let Some((name, rest)) = line.split_once(' ') {
        let desc = rest.trim().trim_matches('"').to_string();
        Ok((name.to_string(), desc))
    } else {
        Ok((line.to_string(), String::new()))
    }
}

fn parse_mode_line(line: &str) -> Option<OutputMode> {
    // `1920x1080 px, 60.000000 Hz (preferred, current)`
    let line = line.trim();
    let (dims, rest) = line.split_once("px")?;
    let dims = dims.trim();
    let (w, h) = dims.split_once('x')?;
    let width: u32 = w.trim().parse().ok()?;
    let height: u32 = h.trim().parse().ok()?;

    let rest = rest.trim().trim_start_matches(',').trim();
    let refresh_part = rest.split("Hz").next()?.trim();
    let refresh_hz: f64 = refresh_part.parse().ok()?;

    let preferred = rest.contains("preferred");
    let current = rest.contains("current");
    Some(OutputMode {
        width,
        height,
        refresh_hz,
        preferred,
        current,
    })
}

fn parse_pos(s: &str) -> Result<(i32, i32)> {
    let (x, y) = s
        .split_once(',')
        .context("posição inválida")?;
    Ok((x.trim().parse()?, y.trim().parse()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"DP-2 "Dell Inc. Dell S2716DG (DP-2)"
  Make: Dell Inc.
  Model: Dell S2716DG
  Physical size: 600x340 mm
  Enabled: yes
  Modes:
    2560x1440 px, 59.951000 Hz (preferred)
    2560x1440 px, 143.964005 Hz (current)
    1920x1080 px, 60.000000 Hz
  Position: 1920,0
  Transform: normal
  Scale: 1.000000
HDMI-A-1 "Ancor Communications Inc VG248 (HDMI-A-1)"
  Enabled: yes
  Modes:
    1920x1080 px, 60.000000 Hz (preferred, current)
  Position: 0,0
  Transform: normal
  Scale: 1.250000
"#;

    #[test]
    fn parse_two_outputs() {
        let heads = parse_wlr_randr_list(SAMPLE).unwrap();
        assert_eq!(heads.len(), 2);
        assert_eq!(heads[0].name, "DP-2");
        assert_eq!(heads[0].pos_x, 1920);
        assert!((heads[0].scale - 1.0).abs() < f64::EPSILON);
        let current = heads[0].current_mode().unwrap();
        assert!((current.refresh_hz - 143.964005).abs() < 0.001);
        assert_eq!(heads[1].name, "HDMI-A-1");
        assert!((heads[1].scale - 1.25).abs() < f64::EPSILON);
        assert_eq!(heads[1].current_mode().unwrap().width, 1920);
    }

    #[test]
    fn mode_cli() {
        let m = OutputMode {
            width: 1920,
            height: 1080,
            refresh_hz: 60.0,
            preferred: true,
            current: true,
        };
        assert_eq!(m.cli_mode(), "1920x1080@60.000000");
    }
}
