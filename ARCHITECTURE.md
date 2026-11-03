# Arquitetura do TUXI

O TUXI é um **Desktop Environment** para Linux com experiência **tradicional**
(painel, menu iniciar, categorias de aplicações), identidade visual própria e
stack baseada em GTK — sem ser uma distribuição Linux e sem fork de GNOME,
Cinnamon ou KDE.

## Princípios

1. **Experiência própria, infraestrutura reutilizada.** Construímos o shell e a
   identidade; reaproveitamos compositor, FreeDesktop, PipeWire, portals, etc.
2. **Documentação oficial como fonte de verdade.** Ver [docs/CONVENTIONS.md](docs/CONVENTIONS.md).
3. **Crescer sem reescrever.** Workspace modular, fronteiras estáveis, ADRs para
   decisões que travam o futuro.

## Camadas

```text
Experiência TUXI     painel, menu, launcher, settings, widgets, CSS
Serviços desktop     D-Bus, atalhos, sessão, notificações (futuro)
Infra gráfica        labwc (compositor oficial) sobre wlroots / Wayland
Linux                kernel, DRM/KMS, PipeWire, systemd, portals
```

Nossa responsabilidade principal está nas duas primeiras camadas. O compositor
é **reutilizado** (labwc), com config de sessão TUXI em `packaging/labwc/`.
Decisão: [docs/decisions/0002-compositor-labwc.md](docs/decisions/0002-compositor-labwc.md).

## O que é nosso vs reutilizado

| Nosso | Reutilizado |
|-------|-------------|
| Painel, menu iniciar, identidade CSS | GTK 4, Libadwaita (componentes) |
| Design system TUXI | Gio / FreeDesktop (apps, categorias) |
| Config/sessão labwc TUXI (`-C`) | **labwc** (compositor oficial) |
| Futuros settings / widgets / sessão TUXI | PipeWire, UPower, BlueZ, xdg-desktop-portal |

## Stack atual (bootstrap)

- **Linguagem:** Rust
- **UI:** GTK 4 + Libadwaita + Relm4 0.11 + gtk4-rs
- **Shell Wayland:** gtk4-layer-shell (painel)
- **Compositor:** labwc (sessão e nested)
- **Estilo:** GTK CSS (`tuxi.css`) sobre Adwaita
- **Descoberta de apps:** `gio::AppInfo` (Desktop Entry Spec)

Detalhes e justificativa: [docs/decisions/0001-stack.md](docs/decisions/0001-stack.md).

## Mapa de crates

### Agora

| Crate | Papel |
|-------|--------|
| `tuxi-panel` | Barra inferior + menu iniciar (binário Relm4) |
| `tuxi-settings` | Central de configurações (Sobre, Telas, placeholders) |
| `tuxi-desktop` | Área de trabalho (ícones XDG Desktop + wallpaper) |

### Crescimento (quando o código pedir extração)

| Crate | Quando extrair | Conteúdo |
|-------|----------------|----------|
| `tuxi-apps` | Menu/launcher estabilizados | Descoberta e launch FreeDesktop |
| `tuxi-core` | Tipos/config compartilhados por ≥2 crates | Erros, config, IDs XDG |
| `tuxi-ui` | Widgets/CSS repetidos entre apps | Tokens, helpers, componentes |
| `tuxi-launcher` | Busca global além do menu | Launcher dedicado |
| `tuxi-session` | Sessão selecionável no login | `.desktop` Wayland + orquestração |

**Regra:** não criar crates vazios. Extrair quando houver fronteira clara e
reuso real. Dependências só “para baixo” (binários → libs; nunca o inverso).

## UX alvo (fase 1)

Desktop tradicional: painel inferior (reposicionável no futuro), botão de menu /
Super, categorias FreeDesktop (Educação, Desenvolvimento, Sistema, …), relógio
e área de status. Não é o fluxo Activities do GNOME.

## Compatibilidade entre distribuições

O TUXI **não é uma distro**. O alvo é rodar como ambiente gráfico em qualquer
Linux que ofereça Wayland + stack FreeDesktop.

| Camada | Como fica portável |
|--------|--------------------|
| Apps no menu | `gio::AppInfo` + arquivos `.desktop` (XDG) — igual em Fedora, Debian, Arch, openSUSE |
| Config | `$XDG_CONFIG_HOME/tuxi/` |
| Painel | gtk4-layer-shell (compositor com `wlr-layer-shell` / equivalente) |
| Empacotamento | **À parte:** RPM, DEB, pacman, etc. — não muda a lógica do desktop |

Não precisamos de código “se Fedora / se Debian” para listar apps. Precisamos
seguir as specs FreeDesktop e, na hora de distribuir, gerar pacotes para as
distros-mãe (Fedora, Debian, Arch, openSUSE). Ubuntu/Mint etc. herdam DEB;
derivados Fedora/Arch/SUSE seguem o pacote da mãe.

Janelas abertas na barra exigem protocolo Wayland (`foreign-toplevel` ou
equivalente do compositor) — isso é da sessão/compositor, não da distro.

## Fora de escopo (bootstrap)

- Compositor próprio (Smithay) ou sessão baseada em Mutter/KWin
- Distro, ISO, pacotes RPM/DEB, CI
- Fork de GNOME Shell, Cinnamon ou KWin
- Biblioteca visual completa do zero (usamos Libadwaita + CSS)
