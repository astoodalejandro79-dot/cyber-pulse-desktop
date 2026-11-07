#!/usr/bin/env bash
# Instala Cyber Pulse como aplicación (menú de apps / acceso directo).
# Uso: ./install.sh [--desktop] [--autostart]
#   --desktop    también crea el acceso directo en el escritorio
#   --autostart  abre el orbe automáticamente al iniciar sesión
set -e
cd "$(dirname "$0")"

[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
if ! command -v cargo >/dev/null 2>&1; then
  echo "[!] Falta Rust. Ejecuta primero ./run.sh (instala todo) y vuelve a correr esto."
  exit 1
fi

echo "[*] Compilando..."
cargo build --release

BIN="$HOME/.local/bin"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor/scalable/apps"
mkdir -p "$BIN" "$APPS" "$ICONS"

install -m 755 target/release/cyber-pulse "$BIN/cyber-pulse"
install -m 644 assets/cyber-pulse.svg "$ICONS/cyber-pulse.svg"

cat > "$APPS/cyber-pulse.desktop" << DESKTOP
[Desktop Entry]
Type=Application
Name=Cyber Pulse
Comment=Visualizador de audio cyberpunk
Exec=$BIN/cyber-pulse
Icon=cyber-pulse
Terminal=false
Categories=AudioVideo;Audio;
DESKTOP
chmod +x "$APPS/cyber-pulse.desktop"

for arg in "$@"; do
  case "$arg" in
    --desktop)
      DESK="$(xdg-user-dir DESKTOP 2>/dev/null || echo "$HOME/Desktop")"
      if [ -d "$DESK" ]; then
        cp "$APPS/cyber-pulse.desktop" "$DESK/"
        chmod +x "$DESK/cyber-pulse.desktop"
        echo "[+] Acceso directo en el escritorio: $DESK"
      fi
      ;;
    --autostart)
      mkdir -p "$HOME/.config/autostart"
      cp "$APPS/cyber-pulse.desktop" "$HOME/.config/autostart/"
      echo "[+] Se abrirá al iniciar sesión."
      ;;
  esac
done

update-desktop-database "$APPS" 2>/dev/null || true
gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
kbuildsycoca6 2>/dev/null || kbuildsycoca5 2>/dev/null || true

echo "[+] Listo. Busca 'Cyber Pulse' en el menú de aplicaciones."
