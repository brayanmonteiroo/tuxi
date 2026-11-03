# Temas TUXI

## `TUXI` (labwc SSD)

Decoração **server-side** das janelas (barra de título, bordas, botões
minimizar/maximizar/fechar).

| Ficheiro | Função |
|----------|--------|
| `TUXI/labwc/themerc` | Cores, padding, hover, sombras |
| `TUXI/labwc/*.xbm` | Máscaras dos botões (recolorem via themerc) |

Selecionado em `packaging/labwc/rc.xml` (`<theme><name>TUXI</name>`).

Tokens alinhados a `crates/tuxi-panel/style/tuxi.css` (bg, accent, danger).

Apps GTK: `GTK_CSD=0` no `environment` da sessão para preferir esta SSD.
Electron/AppImage podem continuar com chrome próprio.
