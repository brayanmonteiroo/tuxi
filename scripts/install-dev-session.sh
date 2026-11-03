#!/usr/bin/env bash
# Instala a sessão TUXI+labwc para aparecer no LightDM / GDM / SDDM.
#
# CRÍTICO: o script e o TryExec vão para /usr/local (não ~/.local).
# O greeter do LightDM corre como user "lightdm" e NÃO consegue
# executar TryExec em /home/<user>/.local (home costuma ser 700) —
# aí a sessão some da lista e só fica "Sessão do Xfce".
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ "${EUID}" -ne 0 ]]; then
  echo "Este instalador precisa de root para o LightDM enxergar a sessão."
  echo "Rode:  sudo $0"
  exit 1
fi

if [[ -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
  TARGET_USER="${SUDO_USER}"
  TARGET_HOME="$(getent passwd "${SUDO_USER}" | cut -d: -f6)"
else
  echo "Use: sudo -u <seuuser> não; use: sudo $0  (com SUDO_USER do teu login)"
  echo "Ex.: faça login como brayan e: sudo ./scripts/install-dev-session.sh"
  TARGET_USER="brayan"
  TARGET_HOME="/home/brayan"
  if [[ ! -d "${TARGET_HOME}" ]]; then
    echo "Ajuste TARGET no script — home não encontrado."
    exit 1
  fi
fi

SESSION_BIN="/usr/local/bin/tuxi-labwc-session"
SHARE_LABWC="/usr/local/share/tuxi/labwc"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PACKAGING_LABWC="${REPO_ROOT}/packaging/labwc"
USER_LABWC="${TARGET_HOME}/.config/tuxi/labwc"

THEME_SRC="${REPO_ROOT}/packaging/themes/TUXI/labwc"
SHARE_THEME="/usr/local/share/themes/TUXI/labwc"
USER_THEME="${TARGET_HOME}/.local/share/themes/TUXI/labwc"

SETTINGS_DESKTOP_SRC="${REPO_ROOT}/packaging/applications/org.tuxi.Settings.desktop"
SETTINGS_BIN_SRC="${REPO_ROOT}/target/release/tuxi-settings"
if [[ ! -x "${SETTINGS_BIN_SRC}" ]]; then
  SETTINGS_BIN_SRC="${REPO_ROOT}/target/debug/tuxi-settings"
fi

install -d /usr/local/bin \
  /usr/local/share/tuxi/labwc \
  /usr/local/share/themes/TUXI/labwc \
  /usr/local/share/applications \
  /usr/share/wayland-sessions \
  /usr/share/xsessions \
  /usr/local/share/wayland-sessions \
  /usr/local/share/xsessions

install -m 755 "${SCRIPT_DIR}/tuxi-labwc-session" "${SESSION_BIN}"

if [[ -x "${SETTINGS_BIN_SRC}" && -f "${SETTINGS_DESKTOP_SRC}" ]]; then
  install -m 755 "${SETTINGS_BIN_SRC}" /usr/local/bin/tuxi-settings
  install -m 644 "${SETTINGS_DESKTOP_SRC}" /usr/local/share/applications/org.tuxi.Settings.desktop
  install -d "${TARGET_HOME}/.local/share/applications"
  install -m 644 "${SETTINGS_DESKTOP_SRC}" \
    "${TARGET_HOME}/.local/share/applications/org.tuxi.Settings.desktop"
  chown "${TARGET_USER}:${TARGET_USER}" \
    "${TARGET_HOME}/.local/share/applications/org.tuxi.Settings.desktop"
else
  echo "aviso: tuxi-settings não compilado — salte app Configurações." >&2
  echo "  cargo build -p tuxi-settings && sudo $0" >&2
fi

# Template labwc oficial (labwc -C).
if [[ ! -f "${PACKAGING_LABWC}/rc.xml" ]]; then
  echo "Falta ${PACKAGING_LABWC}/rc.xml" >&2
  exit 1
fi
install -m 644 "${PACKAGING_LABWC}/rc.xml" "${SHARE_LABWC}/rc.xml"
install -m 644 "${PACKAGING_LABWC}/menu.xml" "${SHARE_LABWC}/menu.xml"
install -m 644 "${PACKAGING_LABWC}/environment" "${SHARE_LABWC}/environment"
install -m 755 "${PACKAGING_LABWC}/autostart" "${SHARE_LABWC}/autostart"

# Tema SSD (barra de título) — labwc procura em share/themes/<nome>/labwc.
if [[ ! -d "${THEME_SRC}" ]]; then
  echo "Falta ${THEME_SRC}" >&2
  exit 1
fi
install -d "${SHARE_THEME}"
install -m 644 "${THEME_SRC}"/* "${SHARE_THEME}/"

# Cópia no home do developer (sessão usa ~/.config/tuxi/labwc).
install -d "${USER_LABWC}" "${USER_THEME}"
install -m 644 "${PACKAGING_LABWC}/rc.xml" "${USER_LABWC}/rc.xml"
install -m 644 "${PACKAGING_LABWC}/menu.xml" "${USER_LABWC}/menu.xml"
install -m 644 "${PACKAGING_LABWC}/environment" "${USER_LABWC}/environment"
install -m 644 "${THEME_SRC}"/* "${USER_THEME}/"
if [[ ! -f "${USER_LABWC}/autostart" ]]; then
  install -m 755 "${PACKAGING_LABWC}/autostart" "${USER_LABWC}/autostart"
fi
chown -R "${TARGET_USER}:${TARGET_USER}" "${TARGET_HOME}/.config/tuxi" \
  "${TARGET_HOME}/.local/share/themes/TUXI"

desktop_body() {
  cat <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=TUXI labwc
Comment=labwc + painel TUXI (desenvolvimento)
Exec=${SESSION_BIN}
TryExec=${SESSION_BIN}
DesktopNames=labwc;TUXI;
Keywords=tuxi;labwc;wayland;
EOF
}

for dir in \
  /usr/share/wayland-sessions \
  /usr/share/xsessions \
  /usr/local/share/wayland-sessions \
  /usr/local/share/xsessions
do
  desktop_body > "${dir}/tuxi-labwc.desktop"
  chmod 644 "${dir}/tuxi-labwc.desktop"
done

# Cópia de conveniência no home (documentação / PATH do user).
install -d "${TARGET_HOME}/.local/bin"
install -m 755 "${SCRIPT_DIR}/tuxi-labwc-session" "${TARGET_HOME}/.local/bin/tuxi-labwc-session"
chown "${TARGET_USER}:${TARGET_USER}" "${TARGET_HOME}/.local/bin/tuxi-labwc-session"

# Preferência de sessão no AccountsService (se existir) — próximo login.
if command -v dbus-send >/dev/null 2>&1; then
  uid="$(id -u "${TARGET_USER}")"
  dbus-send --system --type=method_call --print-reply \
    --dest=org.freedesktop.Accounts \
    "/org/freedesktop/Accounts/User${uid}" \
    org.freedesktop.Accounts.User.SetXSession string:tuxi-labwc \
    >/dev/null 2>&1 || true
fi

echo "OK — sessão instalada para LightDM."
echo "  Exec/TryExec: ${SESSION_BIN}"
echo "  labwc share:  ${SHARE_LABWC}"
echo "  labwc user:   ${USER_LABWC}"
echo "  tema SSD:     ${SHARE_THEME}"
echo "  settings:     /usr/local/bin/tuxi-settings"
echo "  .desktop:     /usr/share/xsessions/tuxi-labwc.desktop"
echo "  .desktop:     /usr/share/wayland-sessions/tuxi-labwc.desktop"
echo
echo "Override opcional: TUXI_LABWC_CONFIG=/caminho/para/dir"
echo
echo "Confira:"
echo "  ls -la ${SESSION_BIN}"
echo "  grep -E 'Exec|TryExec|Name' /usr/share/xsessions/tuxi-labwc.desktop"
echo
echo "Recarregue o greeter (vai cair na tela de login):"
echo "  sudo systemctl restart lightdm"
echo
echo "Na tela de login escolha \"TUXI labwc\" (uma vez). O LightDM lembra."
echo "Painel precisa existir:  sudo -u ${TARGET_USER} bash -lc 'cd ~/tuxi && cargo build -p tuxi-panel'"
echo "Terminal na sessão: TUXI_SESSION_DEV=1 (ou edite autostart em ${USER_LABWC})"
