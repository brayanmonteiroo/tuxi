#!/usr/bin/env bash
# Desenvolvimento NESTED: labwc (compositor oficial) + painel numa janela.
#
# Uso:
#   ./scripts/tuxi-nested.sh              # UMA VEZ — abre a janela labwc
#   ./scripts/tuxi-reload-panel.sh        # depois de mudar código/CSS (janela aberta)
#
# Não precisas dos dois sempre: nested fica aberto; reload só troca o painel.
#
# Isolamento: corre dentro de `dbus-run-session` para o D-Bus do nested não ser
# o do Plasma (Electron/Bruno/etc. deixam de “roubar” a janela do host).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PANEL="${TUXI_PANEL:-${ROOT}/target/debug/tuxi-panel}"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
NESTED_DISPLAY_FILE="${RUNTIME}/tuxi-nested-display"
NESTED_ENV_FILE="${RUNTIME}/tuxi-nested-env"
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
  rm -f "${NESTED_DISPLAY_FILE}" "${NESTED_ENV_FILE}"
  rm -rf "${CFG}"
}
trap cleanup EXIT

cp "${PACKAGING_CFG}/rc.xml" "${CFG}/rc.xml"
cp "${PACKAGING_CFG}/menu.xml" "${CFG}/menu.xml"
if [[ -f "${PACKAGING_CFG}/environment" ]]; then
  cp "${PACKAGING_CFG}/environment" "${CFG}/environment"
fi
if [[ -n "${TUXI_LABWC_CONFIG:-}" && -d "${TUXI_LABWC_CONFIG}" ]]; then
  cp -a "${TUXI_LABWC_CONFIG}/." "${CFG}/" 2>/dev/null || true
fi

# Stubs D-Bus: impedem ativação de KWallet / secrets / portal KDE no bus nested
# (Electron/AppImage pedem org.kde.kwalletd* e o diálogo aparece no labwc).
NESTED_XDG_DATA="${CFG}/xdg-data"
mkdir -p "${NESTED_XDG_DATA}/dbus-1/services"
stub_dbus_service() {
  local name="$1"
  cat > "${NESTED_XDG_DATA}/dbus-1/services/${name}.service" <<EOF
[D-BUS Service]
Name=${name}
Exec=/bin/false
SystemdService=
EOF
}
for svc in \
  org.kde.kwalletd5 \
  org.kde.kwalletd6 \
  org.kde.kwalletd \
  org.kde.kwalletmanager \
  org.kde.secretprompter \
  org.kde.secretservicecompat \
  org.freedesktop.secrets \
  org.freedesktop.impl.portal.desktop.kwallet \
  org.freedesktop.impl.portal.desktop.kde
do
  stub_dbus_service "${svc}"
done

HOST_XDG_DATA_DIRS="${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
export XDG_DATA_DIRS="${NESTED_XDG_DATA}:${HOST_XDG_DATA_DIRS}"
# labwc environment file (lido pelo compositor / filhos).
cat > "${CFG}/environment" <<EOF
# Gerado por tuxi-nested.sh — stubs KWallet + identidade TUXI.
XDG_CURRENT_DESKTOP=TUXI:labwc
XDG_SESSION_DESKTOP=tuxi
XDG_DATA_DIRS=${XDG_DATA_DIRS}
PASSWORD_STORE=basic
CHROME_PASSWORD_STORE=basic
EOF

cat > "${CFG}/autostart" <<'EOF'
#!/usr/bin/env bash
true
EOF
chmod +x "${CFG}/autostart"

cat > "${CFG}/run-panel.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
echo "\${WAYLAND_DISPLAY}" > "${NESTED_DISPLAY_FILE}"
# Env para o reload relançar o painel no mesmo D-Bus/Wayland do nested.
{
  echo "WAYLAND_DISPLAY=\${WAYLAND_DISPLAY}"
  echo "DBUS_SESSION_BUS_ADDRESS=\${DBUS_SESSION_BUS_ADDRESS:-}"
  echo "XDG_RUNTIME_DIR=\${XDG_RUNTIME_DIR:-}"
  echo "XDG_DATA_DIRS=\${XDG_DATA_DIRS:-}"
} > "${NESTED_ENV_FILE}"
echo "tuxi-nested: painel em WAYLAND_DISPLAY=\${WAYLAND_DISPLAY}" >&2
export TUXI_ALLOW_NESTED=1
export XDG_CURRENT_DESKTOP="\${XDG_CURRENT_DESKTOP:-TUXI:labwc}"
export PASSWORD_STORE="\${PASSWORD_STORE:-basic}"
export CHROME_PASSWORD_STORE="\${CHROME_PASSWORD_STORE:-basic}"
if [[ -d "${ROOT}/vendor/prefix/usr/lib64" ]]; then
  export LD_LIBRARY_PATH="${ROOT}/vendor/prefix/usr/lib64\${LD_LIBRARY_PATH:+:\${LD_LIBRARY_PATH}}"
fi
exec "${PANEL}"
EOF
chmod +x "${CFG}/run-panel.sh"

export TUXI_ALLOW_NESTED=1
export WLR_BACKENDS="${WLR_BACKENDS:-wayland}"
export PASSWORD_STORE="${PASSWORD_STORE:-basic}"
export CHROME_PASSWORD_STORE="${CHROME_PASSWORD_STORE:-basic}"
export XDG_CURRENT_DESKTOP="${XDG_CURRENT_DESKTOP:-TUXI:labwc}"
export XDG_SESSION_DESKTOP="${XDG_SESSION_DESKTOP:-tuxi}"
# Evita herdar “sou Plasma” do host para filhos do labwc.
unset KDE_FULL_SESSION KDE_SESSION_VERSION QT_QPA_PLATFORMTHEME DESKTOP_SESSION || true

echo "Abrindo labwc nested + tuxi-panel…"
echo "  1) Este comando: deixa a janela aberta"
echo "  2) Após editar código: ./scripts/tuxi-reload-panel.sh  (só o painel)"
echo "D-Bus isolado: sim (dbus-run-session) + stubs KWallet/secrets."
echo "Fecha a janela labwc para terminar."

run_labwc() {
  exec labwc -C "${CFG}" -s "${CFG}/run-panel.sh"
}

if command -v dbus-run-session >/dev/null 2>&1; then
  exec dbus-run-session -- env \
    --unset=KDE_FULL_SESSION \
    --unset=KDE_SESSION_VERSION \
    --unset=QT_QPA_PLATFORMTHEME \
    --unset=DESKTOP_SESSION \
    TUXI_ALLOW_NESTED=1 \
    WLR_BACKENDS="${WLR_BACKENDS}" \
    XDG_DATA_DIRS="${XDG_DATA_DIRS}" \
    XDG_CURRENT_DESKTOP="${XDG_CURRENT_DESKTOP}" \
    XDG_SESSION_DESKTOP="${XDG_SESSION_DESKTOP}" \
    PASSWORD_STORE="${PASSWORD_STORE}" \
    CHROME_PASSWORD_STORE="${CHROME_PASSWORD_STORE}" \
    labwc -C "${CFG}" -s "${CFG}/run-panel.sh"
else
  echo "aviso: dbus-run-session não encontrado; apps podem abrir no host" >&2
  run_labwc
fi
