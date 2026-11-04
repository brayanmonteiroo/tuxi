#!/usr/bin/env bash
# Recompila e relança painel (+ desktop) no labwc nested (sem fechar a janela).
#
#   ./scripts/tuxi-nested.sh          # UMA VEZ (deixa aberto)
#   ./scripts/tuxi-reload-panel.sh    # a cada mudança de código/CSS
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PANEL="${ROOT}/target/debug/tuxi-panel"
DESKTOP="${ROOT}/target/debug/tuxi-desktop"
SETTINGS="${ROOT}/target/debug/tuxi-settings"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
NESTED_DISPLAY_FILE="${RUNTIME}/tuxi-nested-display"
NESTED_ENV_FILE="${RUNTIME}/tuxi-nested-env"
HOST_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"

cd "${ROOT}"

if [[ -d "${ROOT}/vendor/prefix/usr/lib64" ]]; then
  export LD_LIBRARY_PATH="${ROOT}/vendor/prefix/usr/lib64${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
fi

cargo build -p tuxi-panel -p tuxi-desktop -p tuxi-settings

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

# Segurança: só recusa se não houver labwc nested (-C /tmp/tuxi-nested-*).
# O nome do socket pode coincidir com o do host (wayland-0).
if ! pgrep -a -x labwc 2>/dev/null | rg -q 'labwc -C /tmp/tuxi-nested'; then
  if [[ "${NESTED_DISPLAY}" == "${HOST_DISPLAY}" && -z "${NESTED_DBUS}" ]]; then
    echo "Recusa: sem labwc nested e display=host (${HOST_DISPLAY})." >&2
    exit 1
  fi
fi

if [[ ! -S "${RUNTIME}/${NESTED_DISPLAY}" ]]; then
  echo "Socket ${RUNTIME}/${NESTED_DISPLAY} não existe. Nested morreu?" >&2
  exit 1
fi

# Sincroniza rc.xml do packaging → config -C do labwc nested e reconfigure.
NESTED_LABWC_PID=""
NESTED_LABWC_CFG=""
while read -r pid; do
  [[ -r "/proc/${pid}/cmdline" ]] || continue
  cmd="$(tr '\0' ' ' < "/proc/${pid}/cmdline")"
  if [[ "${cmd}" == *"labwc -C "* ]]; then
    NESTED_LABWC_PID="${pid}"
    NESTED_LABWC_CFG="$(awk -F' -C ' '{print $2}' <<<"${cmd}" | awk '{print $1}')"
    break
  fi
done < <(pgrep -x labwc || true)

if [[ -n "${NESTED_LABWC_CFG}" && -d "${NESTED_LABWC_CFG}" && -f "${ROOT}/packaging/labwc/rc.xml" ]]; then
  cp "${ROOT}/packaging/labwc/rc.xml" "${NESTED_LABWC_CFG}/rc.xml"
  echo "rc.xml → ${NESTED_LABWC_CFG}/rc.xml"
  # Mantém o .desktop do Settings no XDG_DATA do nested (menu Gio).
  if [[ -x "${SETTINGS}" && -f "${ROOT}/packaging/applications/org.tuxi.Settings.desktop" ]]; then
    mkdir -p "${NESTED_LABWC_CFG}/xdg-data/applications"
    {
      sed '/^Exec=/d' "${ROOT}/packaging/applications/org.tuxi.Settings.desktop"
      echo "Exec=${SETTINGS}"
    } > "${NESTED_LABWC_CFG}/xdg-data/applications/org.tuxi.Settings.desktop"
  fi
  NESTED_XDG_DATA="${NESTED_LABWC_CFG}/xdg-data"
  HOST_XDG_DATA_DIRS="${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
  # Garante prefixo nested mesmo se o env file estiver incompleto.
  export XDG_DATA_DIRS="${NESTED_XDG_DATA}:${HOST_XDG_DATA_DIRS}"
  {
    echo "WAYLAND_DISPLAY=${NESTED_DISPLAY}"
    echo "DBUS_SESSION_BUS_ADDRESS=${NESTED_DBUS}"
    echo "XDG_RUNTIME_DIR=${RUNTIME}"
    echo "XDG_DATA_DIRS=${XDG_DATA_DIRS}"
  } > "${NESTED_ENV_FILE}"
  if [[ -n "${NESTED_LABWC_PID}" ]]; then
    if LABWC_PID="${NESTED_LABWC_PID}" labwc --reconfigure 2>/dev/null; then
      echo "labwc --reconfigure OK (PID ${NESTED_LABWC_PID})"
    else
      kill -HUP "${NESTED_LABWC_PID}" 2>/dev/null || true
      echo "labwc HUP (PID ${NESTED_LABWC_PID})"
    fi
  fi
fi

pkill -x tuxi-panel 2>/dev/null || true
pkill -x tuxi-desktop 2>/dev/null || true
pkill -x tuxi-settings 2>/dev/null || true
sleep 0.2

export TUXI_ALLOW_NESTED=1
export WAYLAND_DISPLAY="${NESTED_DISPLAY}"
export TUXI_SETTINGS="${SETTINGS}"
export TUXI_DESKTOP="${DESKTOP}"
# Crítico: sem isto o painel relançado usa o D-Bus do Plasma e apps abrem no host.
if [[ -n "${NESTED_DBUS}" ]]; then
  export DBUS_SESSION_BUS_ADDRESS="${NESTED_DBUS}"
fi
# Sem o xdg-data nested, o menu só vê apps do host (some "Configurações TUXI").
if [[ -z "${XDG_DATA_DIRS:-}" && -n "${NESTED_LABWC_CFG:-}" ]]; then
  export XDG_DATA_DIRS="${NESTED_LABWC_CFG}/xdg-data:/usr/local/share:/usr/share"
fi

echo "Relançando desktop + painel nested WAYLAND_DISPLAY=${NESTED_DISPLAY}"
if [[ -n "${NESTED_DBUS}" ]]; then
  echo "  DBUS_SESSION_BUS_ADDRESS=(dbus do nested)"
fi
if [[ -x "${DESKTOP}" ]]; then
  "${DESKTOP}" &
  echo "OK desktop — PID $!"
  sleep 0.15
fi
"${PANEL}" &
echo "OK painel — PID $!"
