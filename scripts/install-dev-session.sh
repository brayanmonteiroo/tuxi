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

install -d /usr/local/bin \
  /usr/share/wayland-sessions \
  /usr/share/xsessions \
  /usr/local/share/wayland-sessions \
  /usr/local/share/xsessions

install -m 755 "${SCRIPT_DIR}/tuxi-labwc-session" "${SESSION_BIN}"

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
echo "  .desktop:     /usr/share/xsessions/tuxi-labwc.desktop"
echo "  .desktop:     /usr/share/wayland-sessions/tuxi-labwc.desktop"
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
