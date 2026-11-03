# ADR 0001 — Stack inicial do TUXI

## Status

Aceito

## Contexto

Queremos um desktop Linux com experiência tradicional (painel + menu iniciar),
riqueza de configuração inspirada no KDE/Cinnamon, e identidade visual própria,
usando o ecossistema GTK — sem fork de GNOME Shell, Cinnamon ou KWin, e sem
criar uma distribuição.

Precisamos de componentes maduros (diálogos, listas, navegação) sem herdar a
UX do GNOME Activities como produto.

## Decisão

| Camada | Escolha |
|--------|---------|
| Linguagem | Rust |
| Toolkit | GTK 4 |
| Componentes | Libadwaita |
| Bindings | gtk4-rs |
| Arquitetura de UI | Relm4 0.11 |
| Estilo | GTK CSS + tokens TUXI |
| Painel Wayland | gtk4-layer-shell |
| Apps FreeDesktop | Gio (`AppInfo`) |
| Compositor | labwc (ver [0002-compositor-labwc.md](0002-compositor-labwc.md)) |

**Libadwaita** é infraestrutura de componentes, não a identidade do produto.
A cara do TUXI vem do CSS/design system (padrão conceitual Ubuntu/Yaru).

**Granite** não entra nesta fase; Libadwaita é a escolha oficial.

## Consequências

### Positivas

- Reaproveitamos widgets, acessibilidade e padrões já documentados.
- Relm4 organiza o código Elm-style sem substituir GTK/Adw.
- CSS permite identidade própria sem manter um toolkit paralelo.
- Layer-shell permite painel real em compositors que implementam o protocolo
  (ex.: KDE Plasma Wayland, wlroots) — não no GNOME Shell Wayland.

### Negativas / riscos

- Libadwaita carrega stylesheet Adwaita; precisamos sobrescrever com cuidado
  e sem lutar contra APIs não suportadas (novo ADR se necessário).
- Layer-shell limita onde o painel pode ser testado (não GNOME Wayland).
- Compositor oficial é labwc; integração profunda (workspaces, efeitos) evolui
  na família wlroots — ver ADR 0002.

## Referências

- [ARCHITECTURE.md](../../ARCHITECTURE.md)
- [CONVENTIONS.md](../CONVENTIONS.md)
- [Relm4](https://relm4.org/)
- [Libadwaita](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/)
