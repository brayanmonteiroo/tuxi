#!/usr/bin/env bash
# Recompila e relança só o painel no labwc nested (sem fechar a janela).
#
#   ./scripts/tuxi-nested.sh          # UMA VEZ (deixa aberto)
#   ./scripts/tuxi-reload-panel.sh    # a cada mudança de código/CSS
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PANEL="${ROOT}/target/debug/tuxi-panel"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
NESTED_DISPLAY_FILE="${RUNTIME}/tuxi-nested-display"
NESTED_ENV_FILE="${RUNTIME}/tuxi-nested-env"
HOST_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"

cd "${ROOT}"

if [[ -d "${ROOT}/vendor/prefix/usr/lib64" ]]; then
  export LD_LIBRARY_PATH="${ROOT}/vendor/prefix/usr/lib64${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
fi

cargo build -p tuxi-panel

NESTED_DISPLAY=""
NESTED_DBUS=""
if [[ -f "${NESTED_ENV_FILE}" ]]; then
  # shellcheck disable=SC1090
  # Lê WAYLAND_DISPLAY / DBUS do nested (escrito pelo run-panel).
  while IFS='=' read -r key val; do
    case "${key}" in
      WAYLAND_DISPLAY) NESTED_DISPLAY="${val}" ;;
      DBUS_SESSION_BUS_ADDRESS) NESTED_DBUS="${val}" ;;
      XDG_DATA_DIRS) export XDG_DATA_DIRS="${val}" ;;
    esac
  done < "${NESTED_ENV_FILE}"
fi

if [[ -z "${NESTED_DISPLAY}" && -n "${TUXI_NESTED_DISPLAY:-}" ]]; then
  NESTED_DISPLAY="${TUXI_NESTED_DISPLAY}"
fi
if [[ -z "${NESTED_DISPLAY}" && -f "${NESTED_DISPLAY_FILE}" ]]; then
  NESTED_DISPLAY="$(tr -d '[:space:]' < "${NESTED_DISPLAY_FILE}")"
fi

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
  exit 1
fi

if [[ ! -S "${RUNTIME}/${NESTED_DISPLAY}" ]]; then
  echo "Socket ${RUNTIME}/${NESTED_DISPLAY} não existe. Nested morreu?" >&2
  exit 1
fi

pkill -x tuxi-panel 2>/dev/null || true
sleep 0.2

export TUXI_ALLOW_NESTED=1
export WAYLAND_DISPLAY="${NESTED_DISPLAY}"
# Crítico: sem isto o painel relançado usa o D-Bus do Plasma e apps abrem no host.
if [[ -n "${NESTED_DBUS}" ]]; then
  export DBUS_SESSION_BUS_ADDRESS="${NESTED_DBUS}"
fi

echo "Relançando painel nested WAYLAND_DISPLAY=${NESTED_DISPLAY}"
if [[ -n "${NESTED_DBUS}" ]]; then
  echo "  DBUS_SESSION_BUS_ADDRESS=(dbus do nested)"
fi
"${PANEL}" &
echo "OK — PID $!"
