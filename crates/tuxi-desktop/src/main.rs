//! Binário da área de trabalho TUXI.

mod actions;
mod desktop;
mod layout;
mod scan;
mod style;
mod xdg;

use anyhow::Result;
use desktop::DesktopModel;
use relm4::RelmApp;
use tracing_subscriber::EnvFilter;

const APP_ID: &str = "org.tuxi.Desktop";

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let app = RelmApp::new(APP_ID);
    app.run::<DesktopModel>(());
    Ok(())
}
