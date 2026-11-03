use anyhow::{Context, Result};
use relm4::gtk::{self, CssProvider, STYLE_PROVIDER_PRIORITY_APPLICATION};
use tracing::info;

const TUXI_CSS: &str = include_str!("../style/tuxi.css");

pub(crate) fn load_tuxi_stylesheet() -> Result<()> {
    let provider = CssProvider::new();
    provider.load_from_string(TUXI_CSS);
    let display = gtk::gdk::Display::default().context("sem display GDK")?;
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    info!("stylesheet TUXI (desktop) carregado");
    Ok(())
}
