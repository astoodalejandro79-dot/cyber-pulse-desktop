# Cyber Pulse 🚀🎶

Visualizador de audio de escritorio, translúcido y estilo cyberpunk, hecho en **Rust**.
Un orbe de plasma que pulsa con los graves de tu música, con anillos HUD y partículas reactivas.

## Uso rápido

    git clone https://github.com/astoodalejandro79-dot/cyber-pulse-desktop.git
    cd cyber-pulse-desktop
    ./run.sh

`run.sh` instala Rust y las librerías que falten, compila y abre la app.
La primera vez tarda unos minutos compilando.

## Controles

- Clic izquierdo y arrastrar: mover la ventana
- Clic derecho: cerrar

## Audio

Captura automáticamente el **audio del sistema** (lo que suena por tus parlantes)
usando `parec`. Reacciona sobre todo a los graves (bombo y bajos).

Al abrirlo, mira el mensaje de la terminal:

- `capturando el audio del sistema` → todo bien, no hay que hacer nada.
- `usando la entrada de audio por defecto` → no encontró `parec` y está
  escuchando el micrófono. Instala `parec` (ver abajo) o hazlo manualmente.

### Si no detecta el audio del sistema (modo manual)

1. Instala pavucontrol (ver librerías abajo).
2. Abre el visualizador y pon música.
3. Ejecuta `pavucontrol` y ve a la pestaña **Grabación**.
4. Donde diga `cyber-pulse`, cambia el desplegable a **Monitor of ...**
   (tu salida de audio: parlantes o audífonos).

## Librerías necesarias

`run.sh` intenta instalarlas solo, pero por si acaso:

**Arch / CachyOS / Manjaro**

    sudo pacman -S --needed alsa-lib pkgconf base-devel libpulse pavucontrol libxkbcommon wayland mesa

**Debian / Ubuntu / Mint**

    sudo apt install libasound2-dev pkg-config build-essential pulseaudio-utils pavucontrol libxkbcommon-dev libwayland-dev libgl1-mesa-dev

**Fedora**

    sudo dnf install alsa-lib-devel pkgconf gcc pulseaudio-utils pavucontrol libxkbcommon-devel wayland-devel mesa-libGL-devel

**Rust** (si no lo tienes):

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

| Librería | Para qué sirve |
| --- | --- |
| alsa-lib / libasound2-dev | Compilar la entrada de audio |
| pkgconf / pkg-config | Encontrar las librerías al compilar |
| base-devel / build-essential | Compilador y herramientas de build |
| libpulse / pulseaudio-utils | Trae `parec`, captura el audio del sistema |
| pavucontrol | Modo manual (elegir "Monitor of ...") |
| libxkbcommon, wayland, mesa | Ventana y gráficos |

## Ajustes

En `src/main.rs`, arriba del todo:

- `PUNCH`: fuerza del rebote (más alto = más rebote)
- `BASS_HZ`: frecuencia de corte de los graves
- `MAX_EXPANSION`: cuánto crece el orbe

## Acceso directo (menú de aplicaciones)

    ./install.sh

Compila, instala el programa en `~/.local/bin` y crea la entrada **Cyber Pulse**
con su icono en el menú de aplicaciones (Plasma, GNOME, XFCE, etc.).

Opciones:

    ./install.sh --desktop      # además, icono en el escritorio
    ./install.sh --autostart    # abrir el orbe al iniciar sesión
    ./install.sh --desktop --autostart

Para quitarlo: `./uninstall.sh`
