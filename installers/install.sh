#!/usr/bin/env bash
# install.sh — installs code-mole + dependencies (Graphviz, PlantUML)
# Supported: Debian/Ubuntu, Fedora/RHEL, Arch Linux, macOS (Homebrew)
set -euo pipefail

# ── helpers ──────────────────────────────────────────────────────────────────

step()    { echo; echo "==> $*"; }
ok()      { echo "    [ok] $*"; }
warn()    { echo "    [warn] $*" >&2; }
has_cmd() { command -v "$1" &>/dev/null; }

detect_pkg_manager() {
    if   has_cmd apt-get; then echo apt
    elif has_cmd dnf;     then echo dnf
    elif has_cmd yum;     then echo yum
    elif has_cmd pacman;  then echo pacman
    elif has_cmd brew;    then echo brew
    else echo unknown
    fi
}

install_pkg() {
    local pkg="$1"
    case "$PKG_MGR" in
        apt)    sudo apt-get install -y "$pkg" ;;
        dnf)    sudo dnf install -y "$pkg" ;;
        yum)    sudo yum install -y "$pkg" ;;
        pacman) sudo pacman -S --noconfirm "$pkg" ;;
        brew)   brew install "$pkg" ;;
        *)      warn "Unknown package manager — install $pkg manually"; return 1 ;;
    esac
}

PKG_MGR="$(detect_pkg_manager)"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# ── 1. Graphviz ───────────────────────────────────────────────────────────────

step "Checking Graphviz (dot)"

if has_cmd dot; then
    ok "dot already on PATH — skipping."
else
    echo "    Installing Graphviz via $PKG_MGR..."
    case "$PKG_MGR" in
        apt)    install_pkg graphviz ;;
        dnf|yum) install_pkg graphviz ;;
        pacman) install_pkg graphviz ;;
        brew)   brew install graphviz ;;
        *)      warn "Install Graphviz from https://graphviz.org/download/"; exit 1 ;;
    esac
    has_cmd dot && ok "dot installed." || warn "dot not found after install."
fi

# ── 2. PlantUML ───────────────────────────────────────────────────────────────

step "Checking PlantUML"

if has_cmd plantuml; then
    ok "plantuml already on PATH — skipping."
else
    INSTALLED=false

    case "$PKG_MGR" in
        apt)
            echo "    Installing plantuml via apt..."
            sudo apt-get install -y plantuml && INSTALLED=true || true
            ;;
        dnf|yum)
            # Available in Fedora repos
            install_pkg plantuml && INSTALLED=true || true
            ;;
        pacman)
            install_pkg plantuml && INSTALLED=true || true
            ;;
        brew)
            brew install plantuml && INSTALLED=true || true
            ;;
    esac

    if ! $INSTALLED || ! has_cmd plantuml; then
        # Fallback: download jar + create wrapper
        echo "    Falling back: downloading plantuml.jar..."

        if ! has_cmd java; then
            warn "Java not found. PlantUML requires Java >= 8."
            warn "Install via: sudo apt-get install default-jre  (or equivalent)"
        fi

        LOCAL_BIN="${HOME}/.local/bin"
        JAR_PATH="${LOCAL_BIN}/plantuml.jar"
        WRAPPER="${LOCAL_BIN}/plantuml"

        mkdir -p "$LOCAL_BIN"

        JAR_URL="https://github.com/plantuml/plantuml/releases/latest/download/plantuml.jar"
        echo "    Downloading $JAR_URL ..."
        if has_cmd curl; then
            curl -fsSL -o "$JAR_PATH" "$JAR_URL"
        elif has_cmd wget; then
            wget -q -O "$JAR_PATH" "$JAR_URL"
        else
            warn "Neither curl nor wget found — cannot download plantuml.jar"
            exit 1
        fi

        cat > "$WRAPPER" <<'EOF'
#!/usr/bin/env bash
exec java -jar "$(dirname "$(realpath "$0")")/plantuml.jar" "$@"
EOF
        chmod +x "$WRAPPER"

        # Ensure ~/.local/bin is in PATH for this session
        export PATH="${LOCAL_BIN}:${PATH}"

        # Persistent PATH hint
        for rc in "${HOME}/.bashrc" "${HOME}/.zshrc" "${HOME}/.profile"; do
            if [[ -f "$rc" ]] && ! grep -q '\.local/bin' "$rc"; then
                echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$rc"
                echo "    Added ~/.local/bin to $rc"
            fi
        done

        has_cmd plantuml && ok "plantuml wrapper created at $WRAPPER." \
                         || warn "plantuml not on PATH — open a new terminal."
    else
        ok "plantuml installed."
    fi
fi


# ── Done ──────────────────────────────────────────────────────────────────────

echo
echo "Installation complete."
echo "Run: code-mole --help"
echo "If you have troubles executing code-mole ensure code-mole.exe is in your path environment variable."