#!/usr/bin/env bash
# Recompila e relança só o painel no labwc nested (sem fechar a janela).
#
#   ./scripts/tuxi-nested.sh          # uma vez
#   ./scripts/tuxi-reload-panel.sh    # a cada mudança de código/CSS
#
# IMPORTANTE: no host KDE o Plasma costuma ser wayland-1 e o labwc nested
# fica em wayland-0. Nunca assumir que "não-wayland-0" = nested.
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PANEL="${ROOT}/target/debug/tuxi-panel"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
NESTED_DISPLAY_FILE="${RUNTIME}/tuxi-nested-display"
HOST_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"

cd "${ROOT}"

# Vendor gtk4-layer-shell (prefixo local do repo), se existir.
if [[ -d "${ROOT}/vendor/prefix/usr/lib64" ]]; then
  export LD_LIBRARY_PATH="${ROOT}/vendor/prefix/usr/lib64${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
fi

cargo build -p tuxi-panel

NESTED_DISPLAY=""
if [[ -n "${TUXI_NESTED_DISPLAY:-}" ]]; then
  NESTED_DISPLAY="${TUXI_NESTED_DISPLAY}"
elif [[ -f "${NESTED_DISPLAY_FILE}" ]]; then
  NESTED_DISPLAY="$(tr -d '[:space:]' < "${NESTED_DISPLAY_FILE}")"
fi

# Fallback: qualquer socket wayland que NÃO seja o do host atual.
if [[ -z "${NESTED_DISPLAY}" ]]; then
  for sock in "${RUNTIME}"/wayland-*; do
    [[ -S "${sock}" ]] || continue
    base="$(basename "${sock}")"
    [[ "${base}" == *.lock ]] && continue
    if [[ "${base}" != "${HOST_DISPLAY}" ]]; then
      NESTED_DISPLAY="${base}"
      break
    fi
  done
fi

if [[ -z "${NESTED_DISPLAY}" ]]; then
  echo "Nenhum labwc nested encontrado." >&2
  echo "Abre antes: ./scripts/tuxi-nested.sh" >&2
  echo "(host atual: WAYLAND_DISPLAY=${HOST_DISPLAY})" >&2
  exit 1
fi

if [[ "${NESTED_DISPLAY}" == "${HOST_DISPLAY}" ]]; then
  echo "Recusa: nested resolveu para o mesmo display do host (${HOST_DISPLAY})." >&2
  echo "Abre ./scripts/tuxi-nested.sh e tenta de novo." >&2
  exit 1
fi

if [[ ! -S "${RUNTIME}/${NESTED_DISPLAY}" ]]; then
  echo "Socket ${RUNTIME}/${NESTED_DISPLAY} não existe." >&2
  echo "O nested morreu? Abre ./scripts/tuxi-nested.sh" >&2
  exit 1
fi

# Mata só o painel (labwc continua).
pkill -x tuxi-panel 2>/dev/null || true
sleep 0.2

echo "Relançando painel no nested WAYLAND_DISPLAY=${NESTED_DISPLAY} (host=${HOST_DISPLAY})"
TUXI_ALLOW_NESTED=1 WAYLAND_DISPLAY="${NESTED_DISPLAY}" "${PANEL}" &
echo "OK — PID $!"
