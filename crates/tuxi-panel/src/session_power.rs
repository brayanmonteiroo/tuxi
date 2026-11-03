//! Ações de sessão / energia via logind (FreeDesktop).
//!
//! Preferimos `loginctl` (systemd-logind + polkit do assento ativo).

use std::env;
use std::process::Command;

use anyhow::{bail, Context, Result};
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PowerAction {
    Logout,
    Reboot,
    PowerOff,
}

impl PowerAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Logout => "Sair",
            Self::Reboot => "Reiniciar",
            Self::PowerOff => "Desligar",
        }
    }

    pub(crate) fn run(self) -> Result<()> {
        info!(?self, "ação de energia/sessão");
        match self {
            Self::Logout => logout(),
            Self::Reboot => run_loginctl(&["reboot"]).or_else(|_| run_systemctl(&["reboot"])),
            Self::PowerOff => {
                run_loginctl(&["poweroff"]).or_else(|_| run_systemctl(&["poweroff"]))
            }
        }
    }
}

fn logout() -> Result<()> {
    if let Ok(session) = env::var("XDG_SESSION_ID") {
        if !session.is_empty() {
            return run_loginctl(&["terminate-session", &session]);
        }
    }

    // Fallback: encerra todas as sessões do usuário atual.
    let user = env::var("USER").unwrap_or_default();
    if user.is_empty() {
        bail!("USER e XDG_SESSION_ID indisponíveis para logout");
    }
    run_loginctl(&["terminate-user", &user])
}

fn run_loginctl(args: &[&str]) -> Result<()> {
    let status = Command::new("loginctl")
        .args(args)
        .status()
        .context("falha ao executar loginctl")?;
    if status.success() {
        Ok(())
    } else {
        bail!("loginctl {:?} saiu com {status}", args);
    }
}

fn run_systemctl(args: &[&str]) -> Result<()> {
    let status = Command::new("systemctl")
        .args(args)
        .status()
        .context("falha ao executar systemctl")?;
    if status.success() {
        Ok(())
    } else {
        bail!("systemctl {:?} saiu com {status}", args);
    }
}
