#!/bin/sh
set -eu

if [ "$(uname -s)" != Linux ]; then
    echo "Error: solo se admite Linux." >&2
    exit 1
fi

find_missing() {
    missing=""
    for tool in python3 curl ssh-keygen sha256sum grep; do
        if ! command -v "$tool" >/dev/null 2>&1; then
            missing="$missing $tool"
        fi
    done
    if command -v python3 >/dev/null 2>&1 &&
       ! python3 -c 'import sys; sys.exit(sys.version_info < (3, 8))'; then
        missing="$missing python3>=3.8"
    fi
    if command -v ssh-keygen >/dev/null 2>&1 && command -v grep >/dev/null 2>&1 &&
       ! LC_ALL=C ssh-keygen -Y 2>&1 | grep -q 'ssh-keygen -Y verify'; then
        missing="$missing OpenSSH-with-ssh-keygen-Y-verify"
    fi
}

find_missing

if [ -n "$missing" ]; then
    echo "Faltan dependencias:$missing"
    if command -v apt-get >/dev/null 2>&1; then
        manager=apt
        packages="python3 curl openssh-client coreutils grep"
    elif command -v dnf >/dev/null 2>&1; then
        manager=dnf
        packages="python3 curl openssh-clients coreutils grep"
    elif command -v pacman >/dev/null 2>&1; then
        manager=pacman
        packages="python curl openssh coreutils grep"
    else
        echo "Instala manualmente Python 3.8+, curl, OpenSSH, coreutils y grep con tu gestor de paquetes." >&2
        exit 1
    fi
    printf '¿Instalar %s con %s? [s/N] ' "$packages" "$manager"
    read -r answer
    case "$answer" in
        s|S|si|SI|sí|Sí) ;;
        *) echo "Instalación de dependencias cancelada." >&2; exit 1 ;;
    esac
    if [ "$(id -u)" -eq 0 ]; then
        echo "Ejecuta el instalador sin root; se solicitará sudo cuando sea necesario." >&2
        exit 1
    fi
    if ! command -v sudo >/dev/null 2>&1; then
        echo "Instala sudo o instala las dependencias manualmente." >&2
        exit 1
    fi
    case "$manager" in
        apt) sudo -- apt-get install $packages ;;
        dnf) sudo -- dnf install $packages ;;
        pacman) sudo -- pacman -S --needed $packages ;;
    esac
fi

find_missing
if [ -n "$missing" ]; then
    echo "Error: siguen faltando dependencias compatibles:$missing" >&2
    exit 1
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ ! -f "$script_dir/install_release.py" ]; then
    echo "Error: coloca install_release.py junto a install.sh." >&2
    exit 1
fi
exec python3 -I "$script_dir/install_release.py" "$@"
