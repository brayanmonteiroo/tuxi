#!/usr/bin/env bash
# Desenvolvimento NESTED: labwc + painel numa janela (X11/Wayland do host).
#
# Uso (host KDE/XFCE ou dentro da VM no XFCE):
#   cd ~/tuxi && cargo build -p tuxi-panel
#   ./scripts/tuxi-nested.sh
#
# NÃO uses isto como “desktop de verdade”. Para fullscreen no login:
#   sudo ./scripts/install-dev-session.sh && sessão "TUXI labwc"

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

export TUXI_ALLOW_NESTED=1

# Em sessão Wayland do host, labwc aninha via XWayland ou backend próprio.
# DISPLAY/WAYLAND_DISPLAY do host ficam definidos — por isso o allow nested.
echo "Abrindo labwc nested + tuxi-panel (janela)…"
echo "Fecha a janela do labwc para terminar."

(
  # Espera o compositor nested criar o display interno.
  for _ in $(seq 1 50); do
    if [[ -n "${WAYLAND_DISPLAY:-}" ]] || [[ -S "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/wayland-1" ]]; then
      break
    fi
    sleep 0.1
  done
  sleep 0.3
  # Preferir o wayland do labwc nested (wayland-1, wayland-2, …).
  if [[ -z "${TUXI_NESTED_DISPLAY:-}" ]]; then
    for sock in "${XDG_RUNTIME_DIR}"/wayland-*; do
      base="$(basename "${sock}")"
      if [[ "${base}" != "wayland-0" ]]; then
        export WAYLAND_DISPLAY="${base}"
      fi
    done
  else
    export WAYLAND_DISPLAY="${TUXI_NESTED_DISPLAY}"
  fi
  exec "${PANEL}"
) &

exec labwc
