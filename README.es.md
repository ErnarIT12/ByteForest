<p align="center">
  <img src="assets/logo.png" alt="Logotipo de ByteForest" width="120">
</p>

<h1 align="center">ByteForest</h1>

<p align="center"><b>Un analizador de espacio en disco para Windows, rápido y de código abierto: descubre en segundos qué llena tu unidad.</b></p>

<p align="center"><a href="README.md">English</a> | <a href="README.ru.md">Русский</a> | <a href="README.kk.md">Қазақша</a> | <a href="README.zh-CN.md">简体中文</a> | <b>Español</b> | <a href="README.de.md">Deutsch</a> | <a href="README.ja.md">日本語</a></p>

<p align="center">
  <a href="https://github.com/ErnarIT12/byteforest/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/ErnarIT12/byteforest/ci.yml?branch=main&label=CI"></a>
  <a href="https://github.com/ErnarIT12/byteforest/releases"><img alt="Release" src="https://img.shields.io/github/v/release/ErnarIT12/byteforest?include_prereleases"></a>
  <a href="#licencia"><img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue"></a>
  <img alt="Platform: Windows 10/11" src="https://img.shields.io/badge/platform-Windows%2010%2F11-0078D4">
</p>

![Ventana principal de ByteForest: árbol de carpetas junto a un treemap](docs/screenshots/explorer-dark.png)

*Las capturas muestran datos de demostración ficticios creados con [`scripts/make-demo-data.ps1`](scripts/make-demo-data.ps1).*

## Por qué ByteForest

En volúmenes NTFS, ByteForest lee directamente la Master File Table (MFT) en lugar de preguntarle a Windows por cada carpeta, una a una. Esa es la diferencia entre segundos y minutos.

| Análisis | Modo | Entradas | Tiempo |
|---|---|---|---|
| `C:` en un PC con Windows 11 | MFT, con permisos de administrador | ~1,25 millones | ~2,2 s |
| Runner de GitHub Actions | MFT | ~1,2 millones de archivos | 11,6 s |
| Runner de GitHub Actions | Recorrido de directorios | ~1,2 millones de archivos | 541 s |

Sin permisos de administrador, ByteForest recurre a un recorrido normal de directorios, mucho más lento. Son cifras de máquinas concretas, no un benchmark formal; mide las tuyas con `byteforest bench` (consulta [Uso](#uso)).

## Características

- **Análisis de la MFT** de volúmenes NTFS con permisos de administrador.
- **Modo alternativo registro a registro** cuando la lectura directa del volumen está bloqueada (por ejemplo, por drivers anti-cheat): la MFT se lee un registro cada vez a través del sistema de archivos, y sigue siendo rápido.
- **Recorrido de directorios como alternativa** sin permisos de administrador o en volúmenes que no son NTFS.
- **Árbol de carpetas** con tamaño, proporción respecto al padre, número de archivos y fecha de modificación.
- **Treemap squarified** con zoom a una carpeta, migas de pan e información emergente.
- **Selección sincronizada** entre el árbol, el treemap y la barra de estado.
- **Botón «⚡ Fast scan (admin)»**: reinicia ByteForest con permisos de administrador y retoma el análisis automáticamente.
- Opción **«Always run as administrator»**, para que cada inicio sea rápido.
- **Tema claro y oscuro** con la paleta del logotipo y contraste WCAG AA.
- **Herramienta de línea de comandos** para unidades, análisis y mediciones.

![Información emergente del treemap: nombre de archivo, ruta completa, tamaño y proporción de la vista](docs/screenshots/treemap-tooltip.png)

## Instalación

1. Abre la página de [Releases](https://github.com/ErnarIT12/byteforest/releases).
2. Descarga uno de estos:
   - el instalador: `ByteForest_<versión>_x64-setup.exe` (NSIS) o `ByteForest_<versión>_x64_en-US.msi`;
   - la herramienta de línea de comandos portátil: `byteforest-cli-v<versión>-x64.zip`.
3. Si quieres, verifica la descarga con `SHA256SUMS.txt` de la misma versión: `Get-FileHash <archivo> -Algorithm SHA256`.

Las compilaciones **aún no están firmadas**, así que Windows SmartScreen mostrará una advertencia. Elige «Más información» y después «Ejecutar de todas formas» si confías en la descarga.

Requisitos: Windows 10/11 x64 y WebView2 (viene preinstalado en Windows 11; en Windows 10 el instalador lo descarga si falta).

## Uso

**Aplicación.** Elige una unidad y pulsa **Scan**, o pulsa **⚡ Fast scan (admin)** para reiniciar con permisos elevados y analizar de inmediato. Selecciona carpetas en el árbol o rectángulos en el treemap (ambos se mantienen sincronizados). Haz clic en una carpeta del treemap para hacer zoom; vuelve atrás con las migas de pan, la flecha hacia arriba o la rueda del ratón.

**CLI.**

```text
byteforest drives                    # lista las unidades disponibles
byteforest scan C: --top 20          # analiza e imprime los 20 elementos más grandes
byteforest scan C: --top 20 --json   # lo mismo, en JSON
byteforest bench C: --runs 3         # mide el análisis tres veces
```

Ejecuta la CLI desde un terminal con permisos de administrador para usar el escáner MFT.

## Compilar desde el código fuente

Requisitos previos: Rust (la toolchain está fijada en `rust-toolchain.toml`), Visual Studio Build Tools con la toolchain MSVC x64 y un SDK de Windows, y Node.js LTS.

```powershell
git clone https://github.com/ErnarIT12/byteforest.git
cd byteforest
npm --prefix app ci
npm --prefix app run tauri build   # instaladores
cargo test --workspace             # pruebas (compila antes el frontend: npm --prefix app run build)
```

El flujo de desarrollo y las pruebas que requieren administrador están en [CONTRIBUTING.md](CONTRIBUTING.md).

## Arquitectura

```mermaid
flowchart LR
  ntfs[bf-ntfs<br/>análisis NTFS] --> scan[bf-scan<br/>escáneres]
  core[bf-core<br/>árbol + diseño treemap] --> scan
  scan --> cli[bf-cli<br/>CLI byteforest]
  scan --> app[app/src-tauri<br/>comandos Tauri]
  app --> ui[app/src<br/>interfaz React]
```

- `bf-ntfs`: análisis puro de NTFS sobre segmentos de bytes, sin APIs de Windows.
- `bf-core`: modelo de árbol, agregados de tamaño y diseño del treemap, sin APIs de Windows.
- `bf-scan`: `MftScanner` y `WalkScanner`, además de la selección automática de modo.
- `bf-cli`: la herramienta de línea de comandos `byteforest`.
- `app`: shell de Tauri (`src-tauri`) e interfaz en React + TypeScript (`src`).

El árbol analizado se queda en Rust. La interfaz solo pide fragmentos: los hijos de una carpeta (como máximo 1000 nodos) o los rectángulos de una vista del treemap (como máximo 20 000).

## Privacidad

ByteForest funciona totalmente sin conexión. No tiene telemetría y su código no realiza llamadas de red; la ventana de la aplicación solo puede comunicarse con el backend local en Rust (la Content Security Policy no permite nada más). La configuración se guarda en `%APPDATA%\ByteForest\settings.json` y los registros en `%LOCALAPPDATA%\ByteForest\logs`, ambos en tu equipo.

## Hoja de ruta

- Uso de memoria de 120 bytes por entrada o menos (hoy unos 400).
- Búsqueda y ordenación.
- Panel de detalles.
- Mover a la Papelera de reciclaje.
- Interfaz localizada.

## Contribuir

Se agradecen los issues y los pull requests. Lee primero [CONTRIBUTING.md](CONTRIBUTING.md) y el [Código de conducta](CODE_OF_CONDUCT.md).

## Seguridad

Informa de las vulnerabilidades de forma privada, como se describe en [SECURITY.md](SECURITY.md).

## Licencia

Se distribuye, a tu elección, bajo cualquiera de estas licencias:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- licencia MIT ([LICENSE-MIT](LICENSE-MIT))

Salvo que indiques expresamente lo contrario, cualquier contribución enviada intencionadamente para su inclusión en la obra se licenciará de forma doble como arriba, sin términos ni condiciones adicionales.

## Agradecimientos

Inspirado en analizadores de disco como WinDirStat y WizTree.
