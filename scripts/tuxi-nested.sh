#!/usr/bin/env bash
# Desenvolvimento NESTED: labwc + painel numa janela no Fedora KDE (ou XFCE).
#
#   cargo build -p tuxi-panel
#   ./scripts/tuxi-nested.sh
#
# Usa `labwc -s` para o compositor exportar o WAYLAND_DISPLAY certo ao painel.
# Config isolada em /tmp — não usa ~/.config/labwc (autostart da sessão VM).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PANEL="${TUXI_PANEL:-${ROOT}/target/debug/tuxi-panel}"

if [[ ! -x "${PANEL}" ]]; then
  echo "Compile primeiro: cargo build -p tuxi-panel" >&2
  echo "Esperado: ${PANEL}" >&2
  exit 1
fi

if ! command -v labwc >/dev/null 2>&1; then
  echo "Instale labwc: sudo dnf install labwc" >&2
  exit 1
fi

CFG="$(mktemp -d /tmp/tuxi-nested-XXXXXX)"
cleanup() { rm -rf "${CFG}"; }
trap cleanup EXIT

# Autostart vazio: o painel sobe só via -s (com env Wayland do labwc).
cat > "${CFG}/autostart" <<'EOF'
#!/usr/bin/env bash
# nested: sem autostart extra
true
EOF
chmod +x "${CFG}/autostart"

# rc.xml mínimo opcional — labwc usa defaults se faltar.
# Ambiente nested em compositor Wayland do host:
export TUXI_ALLOW_NESTED=1
export WLR_BACKENDS="${WLR_BACKENDS:-wayland}"

echo "Abrindo labwc nested + tuxi-panel…"
echo "Config temporária: ${CFG}"
echo "Fecha a janela do labwc para terminar."

# -s corre o comando DEPOIS do socket Wayland existir (env já corrigido).
exec labwc -C "${CFG}" -s "${PANEL}"
