//! Ações de sessão / energia via logind (FreeDesktop).
//!
//! Preferimos `loginctl` (systemd-logind + polkit do assento ativo).
//! No labwc nested (`TUXI_ALLOW_NESTED`), reboot/poweroff são bloqueados e
//! logout encerra só o compositor (`labwc --exit`).

use std::env;
use std::process::Command;

use anyhow::{bail, Context, Result};
use tracing::{info, warn};

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

    pub(crate) fn icon_name(self) -> &'static str {
        match self {
            Self::Logout => "system-log-out-symbolic",
            Self::Reboot => "system-reboot-symbolic",
            Self::PowerOff => "system-shutdown-symbolic",
        }
    }

    pub(crate) fn tooltip(self) -> &'static str {
        if is_nested_session() {
            match self {
                Self::Logout => "Encerra só o labwc nested (não faz logout do host)",
                Self::Reboot => "Indisponível no nested (não reinicia o PC)",
                Self::PowerOff => "Indisponível no nested (não desliga o PC)",
            }
        } else {
            match self {
                Self::Logout => "Encerrar a sessão e voltar ao login",
                Self::Reboot => "Reiniciar a máquina",
                Self::PowerOff => "Desligar a máquina",
            }
        }
    }

    pub(crate) fn run(self) -> Result<()> {
        info!(
            ?self,
            nested = is_nested_session(),
            "ação de energia/sessão"
        );

        if is_nested_session() {
            return run_nested(self);
        }

        match self {
            Self::Logout => logout(),
            Self::Reboot => run_loginctl(&["reboot"]).or_else(|_| run_systemctl(&["reboot"])),
            Self::PowerOff => run_loginctl(&["poweroff"]).or_else(|_| run_systemctl(&["poweroff"])),
        }
    }
}

/// Sessão nested de desenvolvimento (`TUXI_ALLOW_NESTED=1|true|yes`).
pub(crate) fn is_nested_session() -> bool {
    match env::var("TUXI_ALLOW_NESTED") {
        Ok(v) => {
            let v = v.trim();
            v.eq_ignore_ascii_case("1")
                || v.eq_ignore_ascii_case("true")
                || v.eq_ignore_ascii_case("yes")
        }
        Err(_) => false,
    }
}

/// Reboot/Desligar não devem correr no nested (protegiam o host).
pub(crate) fn power_blocked_in_nested(action: PowerAction) -> bool {
    matches!(action, PowerAction::Reboot | PowerAction::PowerOff)
}

fn run_nested(action: PowerAction) -> Result<()> {
    if power_blocked_in_nested(action) {
        warn!(?action, "power bloqueado no nested");
        bail!(
            "{} bloqueado no labwc nested — não afeta o host. Use a sessão TUXI real para esta ação.",
            action.label()
        );
    }

    // Logout no nested = fechar só o compositor nested.
    info!("nested: a encerrar labwc com --exit");
    let status = Command::new("labwc")
        .arg("--exit")
        .status()
        .context("falha ao executar labwc --exit")?;
    if status.success() {
        Ok(())
    } else {
        bail!("labwc --exit saiu com {status}");
    }
}

fn logout() -> Result<()> {
    if let Ok(session) = env::var("XDG_SESSION_ID") {
        if !session.is_empty() {
            return run_loginctl(&["terminate-session", &session]);
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_action_labels_and_icons() {
        assert_eq!(PowerAction::Logout.label(), "Sair");
        assert_eq!(PowerAction::Reboot.label(), "Reiniciar");
        assert_eq!(PowerAction::PowerOff.label(), "Desligar");
        assert!(PowerAction::Logout.icon_name().contains("log-out"));
        assert!(PowerAction::Reboot.icon_name().contains("reboot"));
        assert!(PowerAction::PowerOff.icon_name().contains("shutdown"));
    }

    #[test]
    fn power_blocked_only_reboot_and_off() {
        assert!(!power_blocked_in_nested(PowerAction::Logout));
        assert!(power_blocked_in_nested(PowerAction::Reboot));
        assert!(power_blocked_in_nested(PowerAction::PowerOff));
    }

    #[test]
    fn is_nested_session_parses_env() {
        // Isolar: guardar e restaurar.
        let prev = env::var("TUXI_ALLOW_NESTED").ok();
        env::remove_var("TUXI_ALLOW_NESTED");
        assert!(!is_nested_session());

        env::set_var("TUXI_ALLOW_NESTED", "1");
        assert!(is_nested_session());
        env::set_var("TUXI_ALLOW_NESTED", "true");
        assert!(is_nested_session());
        env::set_var("TUXI_ALLOW_NESTED", "YES");
        assert!(is_nested_session());
        env::set_var("TUXI_ALLOW_NESTED", "0");
        assert!(!is_nested_session());

        match prev {
            Some(v) => env::set_var("TUXI_ALLOW_NESTED", v),
            None => env::remove_var("TUXI_ALLOW_NESTED"),
        }
    }
}
