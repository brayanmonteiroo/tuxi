# TUXI

Desktop Environment tradicional para Linux (painel + menu iniciar), com stack
**Rust · GTK 4 · Libadwaita · Relm4** e identidade visual própria via CSS.

Não é uma distribuição. Não é um fork do GNOME, Cinnamon ou KDE. Reutilizamos
infraestrutura (Libadwaita, FreeDesktop, layer-shell, compositor do host) e
construímos a experiência TUXI.

## Documentação do projeto

- [ARCHITECTURE.md](ARCHITECTURE.md) — camadas, mapa de crates, escopo
- [docs/CONVENTIONS.md](docs/CONVENTIONS.md) — docs oficiais e regras de engenharia
- [docs/decisions/0001-stack.md](docs/decisions/0001-stack.md) — ADR da stack

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

**Compositor:** o painel usa `wlr-layer-shell` via gtk4-layer-shell. Funciona em
**KDE Plasma Wayland**, Sway, labwc, Hyprland, etc. **Não** funciona no
GNOME Shell Wayland (protocolo não suportado).

### Qualidade

```bash
cargo fmt
cargo clippy -p tuxi-panel -- -D warnings
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
crates/tuxi-panel/   # barra inferior + menu iniciar
docs/                # convenções e ADRs
```

## Apps do ambiente local

Sim: o menu lista as aplicações instaladas na sessão atual (arquivos
`.desktop` via FreeDesktop/Gio). No Fedora KDE, você vê os mesmos apps do
sistema. Em Debian/Arch/openSUSE, a mesma lógica vale — sem fork por distro.

## Próximos passos

- Taskbar real (janelas abertas via foreign-toplevel Wayland)
- Área de trabalho / ícones no desktop
- Launcher dedicado e central de configurações (`adw` + CSS TUXI)
- Extrair descoberta de apps para `tuxi-apps`
- Sessão Wayland selecionável (`org.tuxi.Session`)
- Avaliar compositor (Mutter / Smithay) via backend abstrato
