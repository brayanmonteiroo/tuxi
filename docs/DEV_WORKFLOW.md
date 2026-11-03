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

### Qual script correr?

| Situação | Comando |
|----------|---------|
| Começar a trabalhar | `./scripts/tuxi-nested.sh` **uma vez** (deixa a janela aberta) |
| Mudaste código/CSS | `./scripts/tuxi-reload-panel.sh` (rebuild + só o painel) |
| Fechaste a janela labwc | de novo `./scripts/tuxi-nested.sh` |

Não precisas dos dois em loop. Nested = compositor; reload = painel.

### A) Nested (mesmo compositor do produto)

```bash
cargo build -p tuxi-panel
sudo dnf install -y labwc   # uma vez
./scripts/tuxi-nested.sh    # terminal A — fica aberto
# terminal B, depois de editar:
./scripts/tuxi-reload-panel.sh
```

O nested usa `dbus-run-session` (D-Bus próprio) e grava
`$XDG_RUNTIME_DIR/tuxi-nested-env` para o reload herdar o mesmo bus/Wayland.

**Power:** Reiniciar/Desligar bloqueados; Sair = `labwc --exit`.

**Taskbar:** foreign-toplevel (clique foca; meio fecha).

### Isolar apps do host — nested vs VM

| Nível | O que isola | Limite |
|-------|-------------|--------|
| Nested + dbus-run-session (atual) | Wayland + D-Bus de sessão | Mesmos binários/filesystem do host; app já aberta no Plasma pode “single-instance” |
| Sessão TUXI no login | Sessão gráfica completa | Continua a ver apps instaladas no sistema |
| **VM** (recomendado p/ “DE só dele”) | Kernel, disco, apps, D-Bus | Precisas instalar o que quiseres na VM |

Nested **não** é uma sandbox de apps: reutiliza o que está no Fedora.
Para um desktop “só TUXI” com apps próprias, usa **VM** (ou depois packaging
numa distro). Nested serve para iterar UI/painel depressa no host.

### B) Sessão no login

```bash
git pull && cargo build -p tuxi-panel
sudo ./scripts/install-dev-session.sh
sudo systemctl restart lightdm
```

Escolhe **TUXI labwc** uma vez no greeter. O instalador sincroniza
`~/.config/tuxi/labwc` a partir de `packaging/labwc/`.

## Testes

**Fase 1 (agora):** lógica pura, **dentro dos próprios `.rs`** (não há pasta
`tests/` ainda). Procura por `#[cfg(test)] mod tests` em:

- [`crates/tuxi-panel/src/start_menu.rs`](../crates/tuxi-panel/src/start_menu.rs)
  — categorias, filtro de pesquisa
- [`crates/tuxi-panel/src/session_power.rs`](../crates/tuxi-panel/src/session_power.rs)
  — labels/ícones de energia, guardrail nested

```bash
cargo test -p tuxi-panel
```

**Fase 2 (futuro):** smoke nested manual / script; testes GTK/Relm4 quando a
UI estabilizar. Ver [PERFORMANCE.md](PERFORMANCE.md) para orçamentos.
