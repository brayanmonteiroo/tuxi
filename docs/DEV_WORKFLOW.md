# Como desenvolver um Desktop Environment

## O que Cosmic, GNOME, KDE e Cinnamon fazem

### COSMIC (System76)

- Componentes em Rust; orquestração com `just`.
- **Dia a dia:** compositor em modo aninhado (X11/Winit) — janela dentro do
  desktop atual, igual ao “wlroots - X11” que viste.
- **Integração:** `systemd-sysext` ou sessão instalada, sem estragar a distro base.
- Referência: [cosmic-epoch development](https://deepwiki.com/pop-os/cosmic-epoch/3-development-and-testing),
  [cosmic-comp](https://github.com/pop-os/cosmic-comp).

### GNOME

- Shell (JS) + Mutter; apps GTK/Libadwaita.
- Desenvolvedores usam **JHBuild / Toolbox / VM**, e extensões no GNOME do host.
- Testes pesados de shell: sessão nested (`mutter --nested`) ou VM.
- Quase ninguém “nasce” o DE só em TTY forever — nested + CI + VM.

### KDE Plasma

- Plasma + KWin; apps Qt.
- Iteração rápida: rodar **plasmashell / kwin_wayland --nested** no Plasma já
  instalado, ou `kde-builder` em prefixo próprio (`~/kde`).
- Sessão completa: VM ou segunda sessão no login.

### Cinnamon

- Fork histórico do GNOME Shell; GJS + Muffin.
- Desenvolvimento tipicamente **na própria Mint/Cinnamon** (host ou VM):
  altera applet/extensão, reinicia o shell (`cinnamon --replace` em X11).

## Padrão comum (e o que o TUXI deve copiar)

| Fase | Onde | Para quê |
|------|------|----------|
| 1. Componente | Host ou nested | Painel, CSS, menu — ciclo rápido |
| 2. Sessão | Login gráfico **TUXI labwc** | Sensação de DE, power, layout |
| 3. Integração | VM Fedora dedicada | Sem estragar o KDE do host |
| 4. “Distro com TUXI” | Depois | Quando houver terminal, settings, sessão estável |

Não precisas de “Fedora só com TUXI” no dia 1. Precisas de:

1. **Sessão no login** (sem TTY) — agora.
2. **Terminal + painel** dentro dessa sessão — para desenvolver *lá dentro*.
3. Nested no host só para CSS/UI pontual.

## Por que a sessão sumia no XFCE

O LightDM esconde entradas cujo `TryExec` falha. Com
`TryExec=/home/brayan/.local/bin/...`, o user `lightdm` **não lê** o teu home
(permissão 700) → a sessão não aparece. O instalador coloca o script em
`/usr/local/bin/tuxi-labwc-session`.

## Fluxo recomendado agora

### A) Nested (rápido — o que “faltava”)

No **host** (KDE) ou no XFCE da VM, sem logout:

```bash
cd ~/tuxi   # ou ~/dev/github/tuxi no host
cargo build -p tuxi-panel
sudo dnf install -y labwc   # uma vez
./scripts/tuxi-nested.sh
```

Abre uma janela `wlroots` com a barra TUXI. Ideal para CSS, menu, botões.
Fecha a janela = fim do teste.

Também podes iterar **só o painel** no Plasma (layer-shell), sem labwc:

```bash
cargo run -p tuxi-panel
```

### B) Sessão no login (integração)

```bash
cd ~/tuxi && git pull && cargo build -p tuxi-panel
sudo ./scripts/install-dev-session.sh
sudo systemctl restart lightdm
```

Na tela de login: escolhe **TUXI labwc** uma vez. Depois **Reiniciar** no menu
TUXI deve voltar a essa sessão (LightDM lembra).
