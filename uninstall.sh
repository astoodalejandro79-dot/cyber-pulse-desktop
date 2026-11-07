#!/usr/bin/env bash
rm -f "$HOME/.local/bin/cyber-pulse"
rm -f "$HOME/.local/share/applications/cyber-pulse.desktop"
rm -f "$HOME/.local/share/icons/hicolor/scalable/apps/cyber-pulse.svg"
rm -f "$HOME/.config/autostart/cyber-pulse.desktop"
rm -f "$(xdg-user-dir DESKTOP 2>/dev/null || echo "$HOME/Desktop")/cyber-pulse.desktop"
echo "[+] Cyber Pulse desinstalado."
