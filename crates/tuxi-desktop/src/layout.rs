//! Persistência de posições, wallpaper e geometria da área útil do desktop.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::xdg;

/// Altura do `tuxi-panel` (+ folga). Universal: ícones, modais, widgets.
pub(crate) const PANEL_RESERVE_BOTTOM: i32 = 48 + 8;

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

/// Área útil do ecrã (exclui a barra de tarefas inferior).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkArea {
    pub width: i32,
    pub height: i32,
}

impl WorkArea {
    pub(crate) fn from_screen(screen_w: i32, screen_h: i32) -> Self {
        Self::from_screen_with_panel(screen_w, screen_h, PANEL_RESERVE_BOTTOM)
    }

    pub(crate) fn from_screen_with_panel(screen_w: i32, screen_h: i32, panel_bottom: i32) -> Self {
        Self {
            width: screen_w.max(1),
            height: (screen_h - panel_bottom.max(0)).max(1),
        }
    }

    pub(crate) fn center_child(self, child_w: i32, child_h: i32) -> (f64, f64) {
        center_in_area(self.width, self.height, child_w, child_h)
    }

    pub(crate) fn clamp_child(self, child_w: i32, child_h: i32, x: f64, y: f64) -> (f64, f64) {
        clamp_pos_in_area(self.width, self.height, child_w, child_h, x, y)
    }

    /// O retângulo do filho [x,y,w,h] fica inteiro dentro da área útil.
    #[allow(dead_code)] // API estável p/ widgets futuros / asserts em runtime.
    pub(crate) fn contains_child(self, x: f64, y: f64, child_w: i32, child_h: i32) -> bool {
        let (cx, cy) = self.clamp_child(child_w, child_h, x, y);
        (cx - x).abs() < 0.01
            && (cy - y).abs() < 0.01
            && x >= 0.0
            && y >= 0.0
            && x + f64::from(child_w) <= f64::from(self.width) + 0.01
            && y + f64::from(child_h) <= f64::from(self.height) + 0.01
    }
}

/// Centro do filho dentro da área (nunca negativo).
pub(crate) fn center_in_area(area_w: i32, area_h: i32, child_w: i32, child_h: i32) -> (f64, f64) {
    let aw = f64::from(area_w.max(1));
    let ah = f64::from(area_h.max(1));
    let cw = f64::from(child_w.max(1));
    let ch = f64::from(child_h.max(1));
    (((aw - cw) * 0.5).max(0.0), ((ah - ch) * 0.5).max(0.0))
}

/// Mantém (x,y) para o filho caber na área.
pub(crate) fn clamp_pos_in_area(
    area_w: i32,
    area_h: i32,
    child_w: i32,
    child_h: i32,
    x: f64,
    y: f64,
) -> (f64, f64) {
    let max_x = f64::from((area_w - child_w).max(0));
    let max_y = f64::from((area_h - child_h).max(0));
    (x.clamp(0.0, max_x), y.clamp(0.0, max_y))
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

    #[test]
    fn work_area_subtracts_panel() {
        let wa = WorkArea::from_screen(1920, 1080);
        assert_eq!(wa.width, 1920);
        assert_eq!(wa.height, 1080 - PANEL_RESERVE_BOTTOM);
        assert!(wa.height < 1080);
    }

    #[test]
    fn center_in_work_area_is_middle_not_top() {
        let wa = WorkArea::from_screen(1920, 1080);
        let (x, y) = wa.center_child(400, 200);
        assert!((x - 760.0).abs() < 0.01, "x={x}");
        // Meio vertical da área útil (~1024): (1024-200)/2 ≈ 412
        assert!(y > 300.0, "modal não pode abrir colada ao topo (y={y})");
        assert!(y < 600.0, "y={y}");
        assert!(
            wa.contains_child(x, y, 400, 200),
            "filho centrado tem de caber na área útil"
        );
    }

    #[test]
    fn clamp_never_crosses_panel() {
        let wa = WorkArea::from_screen(1920, 1080);
        let ch = 200;
        let (x, y) = wa.clamp_child(400, ch, 100.0, 50_000.0);
        assert_eq!(x, 100.0);
        assert!(
            y + f64::from(ch) <= f64::from(wa.height) + 0.01,
            "base do widget y+h={} ultrapassa área útil {}",
            y + f64::from(ch),
            wa.height
        );
        assert!(wa.contains_child(x, y, 400, ch));
    }

    #[test]
    fn center_when_area_still_zero_stays_origin() {
        let (x, y) = center_in_area(0, 0, 400, 200);
        assert_eq!((x, y), (0.0, 0.0));
    }

    #[test]
    fn clamp_keeps_modal_on_screen() {
        let (x, y) = clamp_pos_in_area(800, 600, 400, 200, 900.0, -50.0);
        assert_eq!(x, 400.0);
        assert_eq!(y, 0.0);
    }

    #[test]
    fn icon_clamp_respects_panel_like_desktop() {
        // Mesmo contrato que clamp_bounds dos ícones no desktop.
        const ICON_W: i32 = 96;
        const ICON_H: i32 = 100;
        let wa = WorkArea::from_screen(1280, 720);
        let (x, y) = wa.clamp_child(ICON_W, ICON_H, 10.0, 10_000.0);
        assert_eq!(x, 10.0);
        assert!(
            y + f64::from(ICON_H) <= f64::from(wa.height) + 0.01,
            "ícone sob a barra: y+h={} área={}",
            y + f64::from(ICON_H),
            wa.height
        );
        assert_eq!(
            y + f64::from(ICON_H) + f64::from(PANEL_RESERVE_BOTTOM),
            720.0,
            "base do ícone + reserva = ecrã"
        );
    }

    #[test]
    fn oversized_child_pins_to_origin() {
        let wa = WorkArea::from_screen(800, 600);
        let (x, y) = wa.clamp_child(900, 700, 50.0, 50.0);
        assert_eq!((x, y), (0.0, 0.0));
        // contains_child: filho maior que a área — clamp força origem, mas
        // o retângulo não cabe; contains exige caber → false.
        assert!(!wa.contains_child(0.0, 0.0, 900, 700));
    }

    #[test]
    fn custom_panel_reserve() {
        let wa = WorkArea::from_screen_with_panel(1000, 500, 100);
        assert_eq!(wa.height, 400);
        let (x, y) = wa.center_child(200, 100);
        assert_eq!((x, y), (400.0, 150.0));
    }
}
