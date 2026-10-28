#!/usr/bin/env bash
# Installe luxafor-status pour l'utilisateur courant (Ubuntu / Debian).
# Usage : ./install.sh [--no-deps] [--no-udev] [--no-autostart]
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINDIR="$HOME/.local/bin"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
APPDIR="$DATA_HOME/applications"
ICONDIR="$DATA_HOME/luxafor-status/icons"
APPICONDIR="$DATA_HOME/icons/hicolor/scalable/apps"
AUTOSTART="$CONFIG_HOME/autostart"
UDEV_RULE="/etc/udev/rules.d/99-busylights.rules"

DO_DEPS=1; DO_UDEV=1; DO_AUTOSTART=1
for arg in "$@"; do
    case "$arg" in
        --no-deps)      DO_DEPS=0 ;;
        --no-udev)      DO_UDEV=0 ;;
        --no-autostart) DO_AUTOSTART=0 ;;
        -h|--help)
            sed -n '2,3p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Option inconnue : $arg" >&2; exit 2 ;;
    esac
done

step() { printf '\n\033[1;34m==>\033[0m %s\n' "$1"; }
ok()   { printf '    \033[32m✓\033[0m %s\n' "$1"; }
warn() { printf '    \033[33m!\033[0m %s\n' "$1"; }

if [ "$(id -u)" -eq 0 ]; then
    echo "Lance ce script avec ton utilisateur normal (pas en root) ; il demandera sudo si besoin." >&2
    exit 1
fi

# 1. Dépendances système ------------------------------------------------------
if [ "$DO_DEPS" -eq 1 ]; then
    step "Dépendances système"
    PKGS=(pipx python3-gi gir1.2-gtk-3.0 gir1.2-ayatanaappindicator3-0.1 libnotify-bin)
    MISSING=()
    for p in "${PKGS[@]}"; do
        dpkg -s "$p" >/dev/null 2>&1 || MISSING+=("$p")
    done
    if [ "${#MISSING[@]}" -gt 0 ]; then
        echo "    Installation de : ${MISSING[*]}"
        sudo apt-get update -qq
        sudo apt-get install -y "${MISSING[@]}"
    fi
    ok "paquets apt présents"

    step "busylight-for-humans (pilote de la lampe)"
    export PATH="$BINDIR:$PATH"
    if command -v busylight >/dev/null 2>&1; then
        ok "déjà installé"
    else
        pipx install busylight-for-humans
        pipx ensurepath >/dev/null 2>&1 || true
        ok "installé via pipx"
    fi
fi
export PATH="$BINDIR:$PATH"

# 2. Règle udev (accès USB sans sudo) -----------------------------------------
if [ "$DO_UDEV" -eq 1 ]; then
    step "Règle udev (accès USB sans sudo)"
    TMP_RULE="$(mktemp)"
    if command -v busylight >/dev/null 2>&1 \
       && busylight udev-rules -o "$TMP_RULE" >/dev/null 2>&1 \
       && [ -s "$TMP_RULE" ]; then
        ok "règles générées par busylight"
    else
        warn "génération automatique impossible, utilisation de la règle fournie"
        cp "$SRC/udev/99-busylights.rules" "$TMP_RULE"
    fi
    sudo install -m 0644 "$TMP_RULE" "$UDEV_RULE"
    rm -f "$TMP_RULE"
    sudo udevadm control --reload-rules
    sudo udevadm trigger
    ok "règle installée dans $UDEV_RULE"
fi

# 3. Scripts, icônes, lanceurs ------------------------------------------------
step "Installation des fichiers"
mkdir -p "$BINDIR" "$APPDIR" "$ICONDIR" "$APPICONDIR"

install -m 0755 "$SRC/bin/luxafor-toggle" "$BINDIR/luxafor-toggle"
install -m 0755 "$SRC/bin/luxafor-tray"   "$BINDIR/luxafor-tray"
for s in green red yellow off; do
    install -m 0644 "$SRC/share/icons/luxafor-$s.svg" "$ICONDIR/luxafor-$s.svg"
done
install -m 0644 "$SRC/share/icons/luxafor.svg" "$APPICONDIR/luxafor.svg"

sed "s|@BINDIR@|$BINDIR|g" "$SRC/share/applications/luxafor.desktop.in" \
    > "$APPDIR/luxafor.desktop"
update-desktop-database "$APPDIR" >/dev/null 2>&1 || true
gtk-update-icon-cache -q "$DATA_HOME/icons/hicolor" >/dev/null 2>&1 || true
ok "scripts dans $BINDIR"
ok "lanceur « Luxafor » ajouté au menu"

if [ "$DO_AUTOSTART" -eq 1 ]; then
    mkdir -p "$AUTOSTART"
    sed "s|@BINDIR@|$BINDIR|g" "$SRC/share/applications/luxafor-tray.desktop.in" \
        > "$AUTOSTART/luxafor-tray.desktop"
    ok "icône de statut lancée à chaque ouverture de session"
fi

# 4. Démarrage immédiat de l'icône --------------------------------------------
step "Démarrage de l'icône de statut"
pkill -f "$BINDIR/luxafor-tray" >/dev/null 2>&1 || true
if [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
    nohup "$BINDIR/luxafor-tray" >/dev/null 2>&1 &
    disown || true
    ok "icône lancée"
else
    warn "pas de session graphique détectée : l'icône démarrera à la prochaine connexion"
fi

# 5. Vérification -------------------------------------------------------------
step "Vérification"
if command -v busylight >/dev/null 2>&1 && busylight list 2>/dev/null | grep -qi luxafor; then
    ok "Luxafor détectée"
else
    warn "aucune Luxafor détectée — débranche puis rebranche la lampe, puis lance : busylight list"
fi

cat <<EOF

Terminé ✔

  • Menu Budgie → « Luxafor » : glisse-le dans le dock (un clic = statut suivant)
  • Zone de notification : la pastille montre la couleur actuelle (clic = menu)
  • Terminal : luxafor-toggle [green|red|yellow|off]

Désinstallation : ./uninstall.sh
EOF
