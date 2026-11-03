//! Binário da central de configurações TUXI.

mod pages;
mod shell;
mod style;
mod system_info;
// window_controls: reservado se algum dia precisarmos de CSD próprio;
// por agora a SSD do labwc é a fonte de verdade (evita botões duplicados).
#[allow(dead_code)]
mod window_controls;
mod wlr_randr;

use anyhow::Result;
use relm4::RelmApp;
use shell::{parse_initial_page, SettingsModel};
use tracing_subscriber::EnvFilter;

const APP_ID: &str = "org.tuxi.Settings";

fn main() -> Result<()> {
    // Pedir SSD ao compositor (labwc + tema TUXI) antes do GTK inicializar.
    std::env::set_var("GTK_CSD", "0");

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let initial = parse_initial_page();
    let app = RelmApp::new(APP_ID);
    app.run::<SettingsModel>(initial);
    Ok(())
}
