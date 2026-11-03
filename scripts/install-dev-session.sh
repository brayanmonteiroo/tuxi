#!/usr/bin/env bash
# Instala a sessão de desenvolvimento TUXI+labwc.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

if [[ "${EUID}" -eq 0 && -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
  TARGET_HOME="$(getent passwd "${SUDO_USER}" | cut -d: -f6)"
  TARGET_USER="${SUDO_USER}"
else
  TARGET_HOME="${HOME}"
  TARGET_USER="$(id -un)"
fi

BIN_DIR="${TARGET_HOME}/.local/bin"
SESSION_BIN="${BIN_DIR}/tuxi-labwc-session"

mkdir -p "${BIN_DIR}" \
  "${TARGET_HOME}/.local/share/wayland-sessions" \
  "${TARGET_HOME}/.local/share/xsessions"

install -m 755 "${SCRIPT_DIR}/tuxi-labwc-session" "${SESSION_BIN}"
if [[ "${EUID}" -eq 0 && -n "${TARGET_USER}" ]]; then
  chown "${TARGET_USER}:${TARGET_USER}" "${SESSION_BIN}" || true
fi

# Nome sem parênteses — alguns greeters LightDM engasgam com isso.
desktop_body() {
  cat <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=TUXI labwc
Comment=labwc + painel TUXI (desenvolvimento)
Exec=dbus-run-session -- ${SESSION_BIN}
TryExec=${SESSION_BIN}
DesktopNames=labwc;TUXI;
Keywords=tuxi;labwc;wayland;
EOF
}

write_desktop() {
  local dest="$1"
  desktop_body > "${dest}"
  chmod 644 "${dest}"
  if [[ "${EUID}" -eq 0 && -n "${TARGET_USER}" && "${dest}" == "${TARGET_HOME}"/* ]]; then
    chown "${TARGET_USER}:${TARGET_USER}" "${dest}" || true
  fi
}

write_desktop "${TARGET_HOME}/.local/share/wayland-sessions/tuxi-labwc.desktop"
write_desktop "${TARGET_HOME}/.local/share/xsessions/tuxi-labwc.desktop"

if [[ "${EUID}" -eq 0 ]]; then
  install -d /usr/share/wayland-sessions /usr/share/xsessions
  write_desktop /usr/share/wayland-sessions/tuxi-labwc.desktop
  write_desktop /usr/share/xsessions/tuxi-labwc.desktop
  echo "Instalado em /usr/share/{wayland-sessions,xsessions}/"
  echo "Exec → ${SESSION_BIN}"
  echo
  echo "Reinicie o LightDM para recarregar a lista de sessões:"
  echo "  sudo systemctl restart lightdm"
else
  echo "Depois rode: sudo \"${REPO_ROOT}/scripts/install-dev-session.sh\""
fi

echo
echo "Alvo: ${TARGET_USER} (${TARGET_HOME})"
ls -la "${SESSION_BIN}"
ls -la "${TARGET_HOME}/.local/share/xsessions/tuxi-labwc.desktop"
echo
echo "=== Como ver em TELA CHEIA (não janela) ==="
echo "Virt-Manager → Send Key → Ctrl+Alt+F3"
echo "login no TTY →:"
echo "  dbus-run-session -- ${SESSION_BIN}"
echo "Voltar ao login gráfico: Ctrl+Alt+F1 (ou F2)"
