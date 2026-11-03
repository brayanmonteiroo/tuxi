#!/usr/bin/env bash
# Desenvolvimento NESTED: labwc (compositor oficial) + painel numa janela.
#
#   cargo build -p tuxi-panel
#   ./scripts/tuxi-nested.sh
#
# Usa o mesmo template que a sessão: packaging/labwc (labwc -C).
# Clique direito no fundo = menu de emergência do labwc, NÃO do TUXI.
#
# Depois de mudar código/CSS (sem fechar esta janela):
#   ./scripts/tuxi-reload-panel.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PANEL="${TUXI_PANEL:-${ROOT}/target/debug/tuxi-panel}"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
NESTED_DISPLAY_FILE="${RUNTIME}/tuxi-nested-display"
PACKAGING_CFG="${ROOT}/packaging/labwc"

if [[ ! -x "${PANEL}" ]]; then
  echo "Compile primeiro: cargo build -p tuxi-panel" >&2
  echo "Esperado: ${PANEL}" >&2
  exit 1
fi

if ! command -v labwc >/dev/null 2>&1; then
  echo "Instale labwc: sudo dnf install labwc" >&2
  exit 1
fi

if [[ ! -f "${PACKAGING_CFG}/rc.xml" ]]; then
  echo "Falta ${PACKAGING_CFG}/rc.xml" >&2
  exit 1
fi

CFG="$(mktemp -d /tmp/tuxi-nested-XXXXXX)"
cleanup() {
  rm -f "${NESTED_DISPLAY_FILE}"
  rm -rf "${CFG}"
}
trap cleanup EXIT

# Mesma fonte de verdade que a sessão (override via TUXI_LABWC_CONFIG = dir extra).
cp "${PACKAGING_CFG}/rc.xml" "${CFG}/rc.xml"
cp "${PACKAGING_CFG}/menu.xml" "${CFG}/menu.xml"
if [[ -f "${PACKAGING_CFG}/environment" ]]; then
  cp "${PACKAGING_CFG}/environment" "${CFG}/environment"
fi
if [[ -n "${TUXI_LABWC_CONFIG:-}" && -d "${TUXI_LABWC_CONFIG}" ]]; then
  # Overlay opcional: ficheiros do override ganham.
  cp -a "${TUXI_LABWC_CONFIG}/." "${CFG}/" 2>/dev/null || true
fi

cat > "${CFG}/autostart" <<'EOF'
#!/usr/bin/env bash
true
EOF
chmod +x "${CFG}/autostart"

cat > "${CFG}/run-panel.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
echo "\${WAYLAND_DISPLAY}" > "${NESTED_DISPLAY_FILE}"
echo "tuxi-nested: painel em WAYLAND_DISPLAY=\${WAYLAND_DISPLAY}" >&2
export TUXI_ALLOW_NESTED=1
if [[ -d "${ROOT}/vendor/prefix/usr/lib64" ]]; then
  export LD_LIBRARY_PATH="${ROOT}/vendor/prefix/usr/lib64\${LD_LIBRARY_PATH:+:\${LD_LIBRARY_PATH}}"
fi
exec "${PANEL}"
EOF
chmod +x "${CFG}/run-panel.sh"

export TUXI_ALLOW_NESTED=1
export WLR_BACKENDS="${WLR_BACKENDS:-wayland}"

echo "Abrindo labwc nested + tuxi-panel (compositor oficial TUXI)…"
echo "Config: ${CFG} (template packaging/labwc)"
echo "Clique direito no preto = menu do labwc (emergência)."
echo "Barra + Menu = TUXI."
echo "Fecha a janela do labwc para terminar."
echo "Iterar sem fechar: ./scripts/tuxi-reload-panel.sh"

exec labwc -C "${CFG}" -s "${CFG}/run-panel.sh"
