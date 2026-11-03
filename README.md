# TUXI

Desktop Environment tradicional para Linux (painel + menu iniciar), com stack
**Rust · GTK 4 · Libadwaita · Relm4** e identidade visual própria via CSS.

Não é uma distribuição. Não é um fork do GNOME, Cinnamon ou KDE. Reutilizamos
infraestrutura (Libadwaita, FreeDesktop, layer-shell, **labwc**) e construímos
a experiência TUXI.

## Documentação do projeto

- [ARCHITECTURE.md](ARCHITECTURE.md) — camadas, mapa de crates, escopo
- [docs/CONVENTIONS.md](docs/CONVENTIONS.md) — docs oficiais e regras de engenharia
- [docs/PERFORMANCE.md](docs/PERFORMANCE.md) — orçamentos e como medir
- [docs/decisions/0001-stack.md](docs/decisions/0001-stack.md) — ADR da stack
- [docs/decisions/0002-compositor-labwc.md](docs/decisions/0002-compositor-labwc.md) — compositor oficial

## Fontes canônicas (seguir como no Laravel)

| Tecnologia | Doc |
|------------|-----|
| Rust / Cargo | https://doc.rust-lang.org/cargo/ |
| GTK 4 | https://docs.gtk.org/gtk4/ |
| Libadwaita | https://gnome.pages.gitlab.gnome.org/libadwaita/doc/ |
| gtk-rs | https://gtk-rs.org/gtk4-rs/stable/latest/book/ |
| Relm4 | https://relm4.org/ |
| FreeDesktop | https://specifications.freedesktop.org/ |
| gtk4-layer-shell | https://github.com/wmww/gtk4-layer-shell |

## Requisitos (Fedora)

```bash
# Toolchain Rust (rustup)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Dependências de sistema
sudo dnf install gtk4-devel libadwaita-devel gtk4-layer-shell gtk4-layer-shell-devel \
  pkgconf-pkg-config gcc clang
```

## Desenvolvimento

```bash
cargo run -p tuxi-panel
```

**Compositor oficial:** [labwc](https://labwc.github.io/) (ADR 0002). Sessão e
nested usam config TUXI (`packaging/labwc`, `labwc -C`). O painel usa
`wlr-layer-shell`. Compat oportunista em outros compositors com o protocolo
(ex. KWin); **não** no GNOME Shell Wayland.

Override da config: `TUXI_LABWC_CONFIG=/caminho/para/dir`.

### Sessão no login (LightDM / XFCE)

```bash
sudo dnf install -y labwc
cd ~/tuxi && git pull && cargo build -p tuxi-panel
sudo ./scripts/install-dev-session.sh
sudo systemctl restart lightdm
```

Na tela de login escolhe **TUXI labwc** (uma vez). O greeter passa a lembrar —
**Reiniciar** no menu TUXI volta para o TUXI, não para o XFCE.

Detalhes e como outros DEs desenvolvem: [docs/DEV_WORKFLOW.md](docs/DEV_WORKFLOW.md).

### Nested (ciclo rápido, como Cosmic/KWin nested)

```bash
cargo build -p tuxi-panel
./scripts/tuxi-nested.sh          # abre a janela (uma vez)
# ... edita código/CSS ...
./scripts/tuxi-reload-panel.sh    # rebuild + só o painel (sem fechar labwc)
```

Não precisas reiniciar o nested a cada mudança — só o painel.  
Só o painel no Plasma (sem labwc): `cargo run -p tuxi-panel`.

### Qualidade

```bash
cargo fmt
cargo clippy -p tuxi-panel -- -D warnings
cargo test -p tuxi-panel
```

### Super / menu iniciar

O botão **Menu** abre o menu com categorias FreeDesktop e pesquisa. O atalho
Super é registrado na janela do painel (API GTK); em sessão Plasma o compositor
costuma capturar Super globalmente — isso é limitação do host, não do TUXI.
Sessão própria virá em `tuxi-session`.

### Variáveis de log

```bash
RUST_LOG=tuxi_panel=debug cargo run -p tuxi-panel
```

## Estrutura

```text
crates/tuxi-panel/      # barra inferior + menu iniciar
crates/tuxi-settings/   # central de configurações
docs/                   # convenções e ADRs
```

## Apps do ambiente local

Sim: o menu lista as aplicações instaladas na sessão atual (arquivos
`.desktop` via FreeDesktop/Gio). No Fedora KDE, você vê os mesmos apps do
sistema. Em Debian/Arch/openSUSE, a mesma lógica vale — sem fork por distro.
O app **Configurações** (`tuxi-settings`) aparece via
`packaging/applications/org.tuxi.Settings.desktop`.

## Próximos passos

- Rede / Bluetooth / energia / aparência reais no `tuxi-settings`
- Área de trabalho / ícones no desktop
- Launcher dedicado
- Extrair descoberta de apps para `tuxi-apps`
- Endurecer sessão `org.tuxi.Session` (já com labwc)
