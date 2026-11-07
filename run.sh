#!/usr/bin/env bash
set -e
cd "$(dirname "$0")"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[*] Instalando Rust..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"
fi

if [ "$(uname)" = "Linux" ] && ! pkg-config --exists alsa 2>/dev/null; then
  echo "[*] Instalando dependencias de audio (ALSA)..."
  if command -v pacman >/dev/null 2>&1; then
    sudo pacman -S --needed --noconfirm alsa-lib pkgconf base-devel
  elif command -v apt-get >/dev/null 2>&1; then
    sudo apt-get install -y libasound2-dev pkg-config build-essential
  elif command -v dnf >/dev/null 2>&1; then
    sudo dnf install -y alsa-lib-devel pkgconf gcc
  else
    echo "[!] Instala manualmente alsa-lib (dev) y pkg-config."
  fi
fi

# Herramienta para capturar el audio del sistema (parec)
if [ "$(uname)" = "Linux" ] && ! command -v parec >/dev/null 2>&1; then
  echo "[*] Instalando parec (captura del audio del sistema)..."
  if command -v pacman >/dev/null 2>&1; then
    sudo pacman -S --needed --noconfirm libpulse
  elif command -v apt-get >/dev/null 2>&1; then
    sudo apt-get install -y pulseaudio-utils
  elif command -v dnf >/dev/null 2>&1; then
    sudo dnf install -y pulseaudio-utils
  fi
fi

exec cargo run --release
