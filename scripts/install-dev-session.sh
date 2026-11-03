#!/usr/bin/env bash
# Instala a sessão de desenvolvimento TUXI+labwc no usuário atual.
#
# No Fedora XFCE o LightDM lista sobretudo xsessions — por isso instalamos
# em wayland-sessions E xsessions, com caminho absoluto no Exec.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BIN_DIR="${HOME}/.local/bin"
SESSION_BIN="${BIN_DIR}/tuxi-labwc-session"

mkdir -p "${BIN_DIR}" \
  "${HOME}/.local/share/wayland-sessions" \
  "${HOME}/.local/share/xsessions"

install -m 755 "${SCRIPT_DIR}/tuxi-labwc-session" "${SESSION_BIN}"

# .desktop gerado com path absoluto (o login do LightDM nem sempre tem ~/.local/bin no PATH).
desktop_body() {
  cat <<EOF
[Desktop Entry]
Name=TUXI (labwc)
Comment=Compositor labwc + painel TUXI (desenvolvimento)
Exec=dbus-run-session -- ${SESSION_BIN}
TryExec=${SESSION_BIN}
Type=Application
DesktopNames=TUXI
EOF
}

desktop_body > "${HOME}/.local/share/wayland-sessions/tuxi-labwc.desktop"
desktop_body > "${HOME}/.local/share/xsessions/tuxi-labwc.desktop"
chmod 644 \
  "${HOME}/.local/share/wayland-sessions/tuxi-labwc.desktop" \
  "${HOME}/.local/share/xsessions/tuxi-labwc.desktop"

# Cópia em /usr/share (LightDM do XFCE costuma olhar só aqui).
if [[ "${EUID}" -eq 0 ]]; then
  install -d /usr/share/wayland-sessions /usr/share/xsessions
  desktop_body > /usr/share/wayland-sessions/tuxi-labwc.desktop
  desktop_body > /usr/share/xsessions/tuxi-labwc.desktop
  chmod 644 /usr/share/wayland-sessions/tuxi-labwc.desktop \
    /usr/share/xsessions/tuxi-labwc.desktop
  echo "Também instalado em /usr/share/{wayland-sessions,xsessions}/"
else
  echo "Para o LightDM do XFCE enxergar com certeza, rode também:"
  echo "  sudo \"${REPO_ROOT}/scripts/install-dev-session.sh\""
fi

echo
echo "Arquivos:"
ls -la "${SESSION_BIN}"
ls -la "${HOME}/.local/share/xsessions/tuxi-labwc.desktop"
ls -la "${HOME}/.local/share/wayland-sessions/tuxi-labwc.desktop"
echo
echo "1) cargo build -p tuxi-panel  (em ${HOME}/tuxi ou ${REPO_ROOT})"
echo "2) Logout → na tela de login escolha \"TUXI (labwc)\""
echo
echo "Plano B (sem login): Ctrl+Alt+F3, login no TTY, depois:"
echo "  dbus-run-session -- ${SESSION_BIN}"
