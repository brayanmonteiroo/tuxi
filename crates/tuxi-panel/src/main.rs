//! Binário do painel TUXI — barra inferior e menu iniciar.

mod panel;
mod session_power;
mod start_menu;
mod style;

use anyhow::Result;
use panel::PanelModel;
use relm4::RelmApp;
use tracing_subscriber::EnvFilter;

const APP_ID: &str = "org.tuxi.Panel";

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Relm4 com feature libadwaita inicializa Adw ao criar a aplicação.
    // O CSS TUXI é aplicado em `PanelModel::init` (precisa de display GDK).
    let app = RelmApp::new(APP_ID);
    app.run::<PanelModel>(());
    Ok(())
}
