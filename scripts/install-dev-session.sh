#!/usr/bin/env bash
# Instala a sessão de desenvolvimento TUXI+labwc no usuário atual.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

mkdir -p "${HOME}/.local/bin" "${HOME}/.local/share/wayland-sessions"

install -m 755 "${SCRIPT_DIR}/tuxi-labwc-session" "${HOME}/.local/bin/tuxi-labwc-session"
install -m 644 "${REPO_ROOT}/packaging/wayland-sessions/tuxi-labwc.desktop" \
  "${HOME}/.local/share/wayland-sessions/tuxi-labwc.desktop"

# Garante que o Exec resolva via PATH do usuário.
if ! grep -q 'export PATH=.*\.local/bin' "${HOME}/.bash_profile" 2>/dev/null \
  && ! grep -q 'export PATH=.*\.local/bin' "${HOME}/.profile" 2>/dev/null; then
  echo "Nota: confirme que ~/.local/bin está no PATH da sessão gráfica."
fi

echo "Sessão instalada: TUXI (labwc)"
echo "1) Compile:  cd ${REPO_ROOT} && cargo build -p tuxi-panel"
echo "2) Logout e escolha a sessão \"TUXI (labwc)\" na tela de login."
