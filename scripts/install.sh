#!/usr/bin/env bash
# install.sh — installs optional dependencies only (no Rust build)
#   • Graphviz (optional SVG renderer)
#   • Java 17+ JDK  (required for --lang java)
#   • Maven 3.6+    (required to build the Java AST parser)
#   • Builds the Java parser fat-jar  (parsers/java/java-parser.jar)
#
# Use install-all.sh to also build and install the codemole binary.
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
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── 1. Graphviz ───────────────────────────────────────────────────────────────

step "Checking Graphviz (dot)"

if has_cmd dot; then
    ok "dot already on PATH — skipping."
else
    echo "    Installing Graphviz via $PKG_MGR..."
    case "$PKG_MGR" in
        apt)     install_pkg graphviz ;;
        dnf|yum) install_pkg graphviz ;;
        pacman)  install_pkg graphviz ;;
        brew)    brew install graphviz ;;
        *)       warn "Install Graphviz from https://graphviz.org/download/ (optional)"; ;;
    esac
    has_cmd dot && ok "dot installed." || warn "dot not found after install."
fi

# ── 2. Java 17+ ──────────────────────────────────────────────────────────────

step "Checking Java 17+ (required for --lang java)"

install_java() {
    case "$PKG_MGR" in
        apt)     install_pkg openjdk-17-jdk ;;
        dnf|yum) install_pkg java-17-openjdk-devel ;;
        pacman)  install_pkg jdk17-openjdk ;;
        brew)    brew install openjdk@17
                 sudo ln -sfn "$(brew --prefix openjdk@17)/bin/java" /usr/local/bin/java 2>/dev/null || true ;;
        *)       warn "Install Java 17+ from https://adoptium.net and re-run."; return 1 ;;
    esac
}

JAVA_OK=false
if has_cmd java; then
    JAVA_VER=$(java -version 2>&1 | awk -F '"' '/version/{print $2}' | cut -d. -f1)
    if [ "${JAVA_VER:-0}" -ge 17 ] 2>/dev/null; then
        ok "java ${JAVA_VER} — OK"
        JAVA_OK=true
    else
        warn "Java found but version < 17 (got ${JAVA_VER}). Installing 17..."
        install_java && JAVA_OK=true
    fi
else
    echo "    Java not found. Installing OpenJDK 17..."
    install_java && JAVA_OK=true
fi

# ── 3. Maven 3.6+ ────────────────────────────────────────────────────────────

step "Checking Maven (required to build java-parser.jar)"

install_maven() {
    case "$PKG_MGR" in
        apt)     install_pkg maven ;;
        dnf|yum) install_pkg maven ;;
        pacman)  install_pkg maven ;;
        brew)    brew install maven ;;
        *)
            MVN_VER="3.9.7"
            MVN_URL="https://archive.apache.org/dist/maven/maven-3/${MVN_VER}/binaries/apache-maven-${MVN_VER}-bin.tar.gz"
            MVN_DIR="${HOME}/.local/share/apache-maven-${MVN_VER}"
            if [ ! -d "$MVN_DIR" ]; then
                echo "    Downloading Maven ${MVN_VER}..."
                curl -fsSL "$MVN_URL" | tar -xz -C "${HOME}/.local/share/"
            fi
            export PATH="${MVN_DIR}/bin:${PATH}"
            ;;
    esac
}

MVN_OK=false
if has_cmd mvn; then
    ok "mvn $(mvn --version 2>&1 | head -1) — OK"
    MVN_OK=true
else
    echo "    Maven not found. Installing..."
    install_maven && MVN_OK=true
fi

# ── 4. Build java-parser.jar ─────────────────────────────────────────────────

step "Building java-parser.jar"

JAVA_PARSER_DIR="$REPO_ROOT/parsers/java"
JAR_PATH="$JAVA_PARSER_DIR/target/java-parser.jar"

if [ "$JAVA_OK" = true ] && [ "$MVN_OK" = true ]; then
    echo "    Running: mvn -q package -f $JAVA_PARSER_DIR/pom.xml"
    mvn -q package -f "$JAVA_PARSER_DIR/pom.xml" -DskipTests
    if [ -f "$JAR_PATH" ]; then
        ok "java-parser.jar built at $JAR_PATH"
        RELEASE_DIR="$REPO_ROOT/target/release"
        mkdir -p "$RELEASE_DIR"
        cp "$JAR_PATH" "$RELEASE_DIR/java-parser.jar"
        ok "java-parser.jar copied to $RELEASE_DIR"
    else
        warn "Build succeeded but jar not found at expected path."
    fi
else
    warn "Skipping java-parser.jar build (Java or Maven missing)."
    warn "Run manually: cd parsers/java && mvn package"
fi

# ── Done ──────────────────────────────────────────────────────────────────────

echo
echo "Installation complete."
echo "Run: codemole --help"
echo "If you have troubles executing codemole ensure codemole is in your path environment variable."