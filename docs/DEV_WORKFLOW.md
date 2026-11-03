# Como desenvolver um Desktop Environment

## Compositor do produto

O compositor **oficial** do TUXI é **labwc** ([ADR 0002](decisions/0002-compositor-labwc.md)).
Nested e sessão de login usam o mesmo binário e a mesma família de config
(`packaging/labwc/`, `labwc -C`).

Override: `TUXI_LABWC_CONFIG=/caminho/para/dir` (diretório com `rc.xml`, etc.).

## O que Cosmic, GNOME, KDE e Cinnamon fazem

### COSMIC (System76)

- Componentes em Rust; orquestração com `just`.
- **Dia a dia:** compositor em modo aninhado — janela dentro do desktop atual.
- **Integração:** `systemd-sysext` ou sessão instalada, sem estragar a distro base.

### GNOME

- Shell (JS) + Mutter; nested / VM / Toolbox.

### KDE Plasma

- Plasma + KWin; nested ou `kde-builder` em prefixo próprio.

### Cinnamon

- Muffin + shell JS; tipicamente na própria sessão Mint.

## Padrão TUXI

| Fase | Onde | Para quê |
|------|------|----------|
| 1. Componente | Nested labwc (ou painel no host) | Painel, CSS, menu — ciclo rápido |
| 2. Sessão | Login **TUXI labwc** | Sensação de DE, power, layout |
| 3. Integração | VM Fedora dedicada | Sem estragar o KDE do host |
| 4. Distro | Depois | Quando sessão e settings estabilizarem |

## Por que a sessão sumia no XFCE

O LightDM esconde entradas cujo `TryExec` falha. Com path em `$HOME`, o user
`lightdm` não lê o home (700). O instalador usa `/usr/local/bin/tuxi-labwc-session`.

## Fluxo recomendado

### A) Nested (mesmo compositor do produto)

```bash
cargo build -p tuxi-panel
sudo dnf install -y labwc   # uma vez
./scripts/tuxi-nested.sh
```

Iterar sem fechar o nested:

```bash
./scripts/tuxi-reload-panel.sh
```

O nested grava `$XDG_RUNTIME_DIR/tuxi-nested-display`. No KDE o host costuma
ser `wayland-1` e o nested `wayland-0` — o reload usa esse ficheiro (não assume
que nested ≠ `wayland-0`).

Só o painel no Plasma (compat): `cargo run -p tuxi-panel`.

### B) Sessão no login

```bash
git pull && cargo build -p tuxi-panel
sudo ./scripts/install-dev-session.sh
sudo systemctl restart lightdm
```

Escolhe **TUXI labwc** uma vez no greeter. O instalador sincroniza
`~/.config/tuxi/labwc` a partir de `packaging/labwc/`.

## Testes

**Fase 1 (agora):** lógica pura

```bash
cargo test -p tuxi-panel
```

**Fase 2 (futuro):** smoke nested manual / script; testes GTK/Relm4 quando a
UI estabilizar. Ver [PERFORMANCE.md](PERFORMANCE.md) para orçamentos.
