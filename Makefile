# Makefile — centralized task management for codemole
#
# Usage:
#   make install                      Install all dependencies + build everything
#   make build                        Build all components (Rust + Java) in release mode
#   make build profile=debug          Build all components in debug mode
#   make build lang=rust              Build only the Rust binary (release)
#   make build lang=rust profile=debug  Build only the Rust binary (debug)
#   make build lang=java              Build only the Java parser JAR
#   make build lang=python            (no-op until Python parser exists)
#   make build lang=go                (no-op until Go parser exists)
#   make clean                        Remove all build artifacts
#   make help                         Show this message

# ── Configuration ─────────────────────────────────────────────────────────────

REPO_ROOT        := $(CURDIR)
JAVA_PARSER_DIR  := $(REPO_ROOT)/parsers/java
JAR_SRC          := $(JAVA_PARSER_DIR)/target/java-parser.jar

# Build profile: release (default) or debug
profile          ?= release

ifeq ($(profile),release)
    CARGO_FLAGS  := --release
    PROFILE_DIR  := release
else ifeq ($(profile),debug)
    CARGO_FLAGS  :=
    PROFILE_DIR  := debug
else
    $(error Unknown profile '$(profile)'. Valid values: release  debug)
endif

JAR_DEST         := $(REPO_ROOT)/target/$(PROFILE_DIR)/java-parser.jar
RUST_BINARY      := $(REPO_ROOT)/target/$(PROFILE_DIR)/codemole

# Optional: install destination (cargo install uses ~/.cargo/bin by default)
CARGO_BIN        := $(HOME)/.cargo/bin

# Detect OS — used to delegate install to the right script.
# On Windows, Chocolatey's make runs under MSYS2 sh.exe, which mangles paths
# when calling Windows batch files (.cmd). Use PowerShell for Maven calls to
# bypass MSYS2 path translation entirely (same approach as the install scripts).
ifeq ($(OS),Windows_NT)
    INSTALL_SCRIPT     := powershell -ExecutionPolicy Bypass -File scripts/install.ps1
    INSTALL_ALL_SCRIPT := powershell -ExecutionPolicy Bypass -File scripts/install-all.ps1
    MVN_BUILD_JAVA     := cmd /c scripts\\build-java.bat $(PROFILE_DIR)
    MVN_CLEAN_JAVA     := cmd /c scripts\\clean-java.bat
else
    INSTALL_SCRIPT     := bash scripts/install.sh
    INSTALL_ALL_SCRIPT := bash scripts/install-all.sh
    MVN_BUILD_JAVA     := mvn -q -f parsers/java/pom.xml package -DskipTests
    MVN_CLEAN_JAVA     := mvn -q -f parsers/java/pom.xml clean
endif

# ── Default goal ──────────────────────────────────────────────────────────────

.DEFAULT_GOAL := help

# ── Help ──────────────────────────────────────────────────────────────────────

.PHONY: help
help:
	@echo ""
	@echo "codemole — available targets"
	@echo ""
	@echo "  make install                          Install dependencies and build all components"
	@echo "  make build                            Build all components in release mode"
	@echo "  make build profile=debug              Build all components in debug mode"
	@echo "  make build lang=rust                  Build only the Rust binary (release)"
	@echo "  make build lang=rust profile=debug    Build only the Rust binary (debug)"
	@echo "  make build lang=java                  Build only the Java parser JAR"
	@echo "  make build lang=python                (reserved — no Python parser yet)"
	@echo "  make build lang=go                    (reserved — no Go parser yet)"
	@echo "  make clean                            Remove Rust and Java build artifacts"
	@echo "  make clean lang=rust                  Remove only Rust artifacts"
	@echo "  make clean lang=java                  Remove only Java artifacts"
	@echo "  make help                             Show this message"
	@echo ""

# ── Install ───────────────────────────────────────────────────────────────────

# 'install' installs deps AND builds everything (mirrors install-all.sh/.ps1)
.PHONY: install
install:
	@echo ""
	@echo "==> Installing dependencies and building codemole..."
	$(INSTALL_ALL_SCRIPT)

# 'install-deps' installs deps only, without building the Rust binary
.PHONY: install-deps
install-deps:
	@echo ""
	@echo "==> Installing dependencies (no Rust build)..."
	$(INSTALL_SCRIPT)

# ── Build ─────────────────────────────────────────────────────────────────────

# Dispatch: if lang is set, build only that language; otherwise build all.
.PHONY: build
build:
ifdef lang
	$(MAKE) _build-$(lang) profile=$(profile)
else
	$(MAKE) _build-all profile=$(profile)
endif

.PHONY: _build-all
_build-all: _build-rust _build-java
	@echo ""
	@echo "==> All components built successfully."

# ── Rust ──────────────────────────────────────────────────────────────────────

.PHONY: _build-rust
_build-rust:
	@echo ""
	@echo "==> Building Rust binary ($(profile))..."
	cargo build $(CARGO_FLAGS)
	@echo "    binary: $(RUST_BINARY)"

# ── Java ──────────────────────────────────────────────────────────────────────

.PHONY: _build-java
_build-java:
	@echo ""
	@echo "==> Building Java parser JAR..."
	$(MVN_BUILD_JAVA)
ifeq ($(OS),Windows_NT)
	@echo "    jar:    target/$(PROFILE_DIR)/java-parser.jar"
else
	@mkdir -p target/$(PROFILE_DIR)
	@cp parsers/java/target/java-parser.jar target/$(PROFILE_DIR)/java-parser.jar
	@echo "    jar:    target/$(PROFILE_DIR)/java-parser.jar"
endif

# ── Python (reserved) ─────────────────────────────────────────────────────────

.PHONY: _build-python
_build-python:
	@echo ""
	@echo "==> [python] No build step required — normalize.py is used directly."
	@echo "    Script: parsers/python/normalize.py"

# ── Go (reserved) ─────────────────────────────────────────────────────────────

.PHONY: _build-go
_build-go:
	@echo ""
	@echo "==> Building Go normalizer..."
	@if command -v go >/dev/null 2>&1; then \
	    go build -o $(REPO_ROOT)/target/$(PROFILE_DIR)/go-normalize $(REPO_ROOT)/parsers/go/normalize.go; \
	    echo "    binary: $(REPO_ROOT)/target/$(PROFILE_DIR)/go-normalize"; \
	else \
	    echo "    [warn] go not found — skipping Go normalizer build."; \
	fi

# Guard: catch unknown lang= values with a clear error
_build-%:
	@echo ""
	@echo "Error: unknown lang '$*'. Valid values: rust  java  python  go"
	@exit 1

# ── Clean ─────────────────────────────────────────────────────────────────────

.PHONY: clean
clean:
ifdef lang
	$(MAKE) _clean-$(lang)
else
	$(MAKE) _clean-all
endif

.PHONY: _clean-all
_clean-all: _clean-rust _clean-java
	@echo ""
	@echo "==> Clean complete."

.PHONY: _clean-rust
_clean-rust:
	@echo ""
	@echo "==> Cleaning Rust artifacts..."
	cargo clean

.PHONY: _clean-java
_clean-java:
	@echo ""
	@echo "==> Cleaning Java artifacts..."
	$(MVN_CLEAN_JAVA)

_clean-%:
	@echo ""
	@echo "Error: unknown lang '$*'. Valid values: rust  java  python  go"
	@exit 1
