# Cyber Pulse 🚀🎶

Visualizador de audio de escritorio, translúcido y estilo cyberpunk, hecho en **Rust**.
Un orbe de plasma que pulsa con el sonido, con anillos HUD y partículas reactivas.

## Uso

    git clone https://github.com/astoodalejandro79-dot/cyber-pulse-desktop.git
    cd cyber-pulse-desktop
    ./run.sh

`run.sh` instala Rust y las librerías de audio si faltan, compila y abre la app.

## Controles

- Clic izquierdo y arrastrar: mover la ventana
- Clic derecho: cerrar

## Audio

Captura automáticamente el **audio del sistema** (lo que suena por tus parlantes),
sin configurar nada. Usa `parec` (PipeWire/PulseAudio). Si no está disponible,
cae a la entrada de audio por defecto.

Reacciona sobre todo a los **graves** (bombo y bajos). Puedes ajustar la sensibilidad
en `src/main.rs`: `BASS_HZ` (frecuencia de corte) y `PUNCH` (fuerza del rebote).
