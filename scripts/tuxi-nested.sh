#!/usr/bin/env bash
# Desenvolvimento NESTED: labwc + painel numa janela no Fedora KDE (ou XFCE).
#
#   cargo build -p tuxi-panel
#   ./scripts/tuxi-nested.sh
#
# Clique direito no fundo preto = menu do LABWC (compositor), NÃO do TUXI.
# O TUXI por agora é a barra + menu iniciar.
#
# Depois de mudar código/CSS (sem fechar esta janela):
#   ./scripts/tuxi-reload-panel.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PANEL="${TUXI_PANEL:-${ROOT}/target/debug/tuxi-panel}"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
NESTED_DISPLAY_FILE="${RUNTIME}/tuxi-nested-display"

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
cleanup() {
  rm -f "${NESTED_DISPLAY_FILE}"
  rm -rf "${CFG}"
}
trap cleanup EXIT

cat > "${CFG}/autostart" <<'EOF'
#!/usr/bin/env bash
true
EOF
chmod +x "${CFG}/autostart"

# Menu de fundo = labwc (deixa explícito que ainda não é o desktop TUXI).
cat > "${CFG}/menu.xml" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<openbox_menu>
  <menu id="root-menu" label="labwc (não é TUXI)">
    <item label="Fechar nested">
      <action name="Exit"/>
    </item>
  </menu>
</openbox_menu>
EOF

# O -s do labwc corre JÁ com WAYLAND_DISPLAY do nested (ex.: wayland-0
# enquanto o Plasma do host é wayland-1). Grava isso para o reload.
cat > "${CFG}/run-panel.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
echo "\${WAYLAND_DISPLAY}" > "${NESTED_DISPLAY_FILE}"
echo "tuxi-nested: painel em WAYLAND_DISPLAY=\${WAYLAND_DISPLAY}" >&2
export TUXI_ALLOW_NESTED=1
exec "${PANEL}"
EOF
chmod +x "${CFG}/run-panel.sh"

export TUXI_ALLOW_NESTED=1
export WLR_BACKENDS="${WLR_BACKENDS:-wayland}"

echo "Abrindo labwc nested + tuxi-panel…"
echo "Clique direito no preto = menu do labwc (compositor)."
echo "Barra + Menu = TUXI."
echo "Fecha a janela do labwc para terminar."
echo "Iterar sem fechar: ./scripts/tuxi-reload-panel.sh"

exec labwc -C "${CFG}" -s "${CFG}/run-panel.sh"
