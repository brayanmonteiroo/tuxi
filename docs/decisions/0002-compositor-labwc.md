# ADR 0002 — Compositor oficial: labwc

## Status

Aceito

## Contexto

O TUXI é um Desktop Environment com shell **externo** ao compositor: painel e
menu em GTK 4 + Relm4 via `wlr-layer-shell` (gtk4-layer-shell). A experiência
alvo é desktop tradicional (stacking), com maturidade de produto a longo prazo
— não um protótipo descartável.

Candidatos avaliados:

| Opção | Problema para o TUXI |
|-------|----------------------|
| Mutter / Muffin / Gala | Shell acoplado ao GNOME/Cinnamon/elementary; sem caminho saudável para painel layer-shell externo |
| KWin | Maduro, mas ecossistema Plasma; TUXI ficaria hóspede do stack KDE |
| cosmic-comp (Smithay) | Feito para o Cosmic (iced); compositor próprio em Rust = outro produto e anos de manutenção |
| Smithay “do zero” | Biblioteca, não compositor; atrasa o shell sem dar completude de DE no dia 1 |
| labwc (wlroots) | Stacking, leve, layer-shell first-class, usado em produção (ex.: Raspberry Pi Desktop, sessões LXQt Wayland) |

## Decisão

**Compositor oficial da sessão TUXI: labwc.**

- Configuração versionada em `packaging/labwc/`, carregada com `labwc -C`.
- Nested de desenvolvimento e sessão de login usam o **mesmo** compositor e a
  mesma família de config (fonte de verdade no repositório).
- Override: variável de ambiente `TUXI_LABWC_CONFIG` (diretório passado a `-C`).

Família permanente: protocolos estilo **wlroots** (layer-shell, e depois
foreign-toplevel, etc.). Clientes do shell TUXI assumem essa família.

## Consequências

### Positivas

- Alinha o painel já existente com a sessão de produto.
- Evita reescrever o DE para o modelo GNOME Shell / Plasma.
- Permite evoluir o shell (painel, settings, launcher) sem manter um compositor.
- Migração futura para outro WM **da mesma família** (ou compositor próprio
  wlroots/Smithay), se um dia faltar capacidade *de compositor*, não quebra o
  paradigma do shell externo.

### Negativas / riscos

- labwc é minimalista: efeitos/animações avançadas não são o foco do projeto.
- Integração shell↔compositor via protocolos Wayland (sem IPC D-Bus rico do labwc).
- “Correr em KWin” pode funcionar para o painel, mas **não** é sessão suportada.

### Explicitamente fora

- Mutter/Muffin como alvo do shell TUXI.
- Desenvolver compositor próprio nesta fase.
- Tratar labwc como “temporário para MVP”.

## Testes (fase 2, documentado)

Fase 1: testes unitários de lógica pura (`cargo test -p tuxi-panel`).  
Fase 2 (futuro): smoke nested manual / script; testes GTK quando fizer sentido.
Não bloqueiam esta ADR.

## Referências

- [labwc](https://labwc.github.io/)
- [ARCHITECTURE.md](../../ARCHITECTURE.md)
- [ADR 0001 — Stack](0001-stack.md)
- [DEV_WORKFLOW.md](../DEV_WORKFLOW.md)
