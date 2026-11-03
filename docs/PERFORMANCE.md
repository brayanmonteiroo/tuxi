# Performance — orçamentos TUXI

Medir antes de otimizar. Spans `tracing` em caminhos quentes; sem dependência
extra de métricas nesta fase.

## Orçamentos iniciais

| Métrica | Orçamento | Como ver |
|---------|-----------|----------|
| Arranque até log `painel TUXI iniciado` | &lt; 500 ms (orientação, SSD) | `RUST_LOG=info` / tempo de wall |
| `filter_apps` ~500 entradas | &lt; 5 ms típico | span `filter_apps` com `RUST_LOG=tuxi_panel=debug` |
| `load_apps_by_category` | registar baseline; sem teto rígido | span `load_apps_by_category` |
| RSS `tuxi-panel` idle | baseline; sem teto rígido ainda | `ps -o rss= -p $(pgrep -x tuxi-panel)` |

## Como instrumentar

```bash
RUST_LOG=tuxi_panel=debug cargo run -p tuxi-panel
# ou no nested, após reload:
RUST_LOG=tuxi_panel=debug ./scripts/tuxi-reload-panel.sh
```

Spans atuais: `load_apps_by_category`, `filter_apps`.

## Testes

```bash
cargo test -p tuxi-panel
```

Lógica de filtro/categorias é coberta por testes unitários (fase 1). Smoke
nested e GTK ficam para fase 2 ([DEV_WORKFLOW.md](DEV_WORKFLOW.md)).
