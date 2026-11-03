#!/usr/bin/env bash
# Recompila e relança só o painel no labwc nested (sem fechar a janela).
#
#   ./scripts/tuxi-nested.sh          # uma vez
#   ./scripts/tuxi-reload-panel.sh    # a cada mudança de código/CSS
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PANEL="${ROOT}/target/debug/tuxi-panel"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"

cd "${ROOT}"
cargo build -p tuxi-panel

# Encontra o Wayland do labwc nested (não o do Plasma = wayland-0).
NESTED_DISPLAY=""
if [[ -n "${TUXI_NESTED_DISPLAY:-}" ]]; then
  NESTED_DISPLAY="${TUXI_NESTED_DISPLAY}"
else
  for sock in "${RUNTIME}"/wayland-*; do
    [[ -S "${sock}" ]] || continue
    base="$(basename "${sock}")"
    if [[ "${base}" != "wayland-0" ]]; then
      NESTED_DISPLAY="${base}"
    fi
  done
fi

if [[ -z "${NESTED_DISPLAY}" ]]; then
  echo "Nenhum labwc nested encontrado." >&2
  echo "Abre antes: ./scripts/tuxi-nested.sh" >&2
  exit 1
fi

# Mata só o painel (labwc continua).
pkill -x tuxi-panel 2>/dev/null || true
sleep 0.2

echo "Relançando painel em WAYLAND_DISPLAY=${NESTED_DISPLAY}"
WAYLAND_DISPLAY="${NESTED_DISPLAY}" "${PANEL}" &
echo "OK — PID $!"
