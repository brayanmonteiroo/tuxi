# Convenções do TUXI

Este documento é o equivalente operacional de “seguir a documentação do
Laravel”: cada tecnologia tem uma fonte canônica; o código e as decisões
devem alinhá-las.

## Fontes canônicas

| Tecnologia | Fonte |
|------------|--------|
| Rust / Cargo workspaces | [The Cargo Book](https://doc.rust-lang.org/cargo/) |
| GTK 4 | [GTK 4 documentation](https://docs.gtk.org/gtk4/) |
| Libadwaita | [Libadwaita docs](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/) |
| gtk-rs / Gio | [gtk-rs book](https://gtk-rs.org/gtk4-rs/stable/latest/book/) e [docs.rs](https://docs.rs/) |
| Relm4 | [Relm4](https://relm4.org/) |
| FreeDesktop | [specifications.freedesktop.org](https://specifications.freedesktop.org/) |
| gtk4-layer-shell | [wmww/gtk4-layer-shell](https://github.com/wmww/gtk4-layer-shell) e protocolo `wlr-layer-shell` |

O GNOME HIG é **referência**, não lei de UX do TUXI. Nossa experiência é
desktop tradicional; a identidade visual é o Design System TUXI via CSS.

## Regras

1. **Antes de inventar API própria**, verificar se Gio, GTK, Libadwaita ou
   Relm4 já resolvem o problema na documentação oficial.
2. **Gambiarra fora da doc** só com ADR em `docs/decisions/` explicando
   porquê, riscos e condição de remoção.
3. **Um crate por responsabilidade.** Dependências apenas de apps/binários
   para libs — nunca o inverso.
4. **API pública mínima:** `pub` só o necessário; preferir `pub(crate)` ou
   privado. Evita acoplamento quando o monorepo crescer.
5. **IDs e paths FreeDesktop/XDG:**
   - Application IDs: `org.tuxi.*` (ex.: `org.tuxi.Panel`)
   - Config: `$XDG_CONFIG_HOME/tuxi/` (padrão `~/.config/tuxi/`)
   - Não inventar hierarquias fora do [XDG Base Directory](https://specifications.freedesktop.org/basedir-spec/latest/).
6. **Erros e logging:** `anyhow` / `thiserror` onde apropriado; `tracing`
   para logs — não espalhar `println!` / `eprintln!` de diagnóstico.
7. **Comentários** explicam o *porquê* específico do TUXI; o *como* deve
   seguir o padrão da documentação da biblioteca.
8. **Qualidade:** `cargo fmt` e `cargo clippy -- -D warnings` antes de
   considerar uma mudança pronta.

## Módulos no `tuxi-panel` (bootstrap)

| Módulo | Responsabilidade |
|--------|------------------|
| `main` | Application ID, init Adw, CSS, run Relm4 |
| `panel` | Layer-shell + layout da barra |
| `start_menu` | Categorias FreeDesktop + launch |
| `style` | Carregar stylesheet TUXI |

Quando `start_menu` estabilizar e for reutilizado, extrair para `tuxi-apps`
conforme [ARCHITECTURE.md](../ARCHITECTURE.md).

## ADRs

Decisões que travam o futuro (stack, compositor, sessão, IPC) vão em
`docs/decisions/NNNN-titulo.md`. Modelo mínimo: contexto, decisão,
consequências.
