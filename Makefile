# Parley developer tasks. Run `make` (or `make help`) to list them.
#
# Works with GNU make on Linux, macOS and Windows (install with `winget install ezwinports.make`,
# or run the same commands from Git Bash). Recipes are single commands so they run in any shell.

APP := parley-app
CORE := parley-core
UI := parley-ui-egui

ifeq ($(OS),Windows_NT)
  PLATFORM := windows
else ifeq ($(shell uname -s),Darwin)
  PLATFORM := macos
else
  PLATFORM := linux
endif

# Under WSL there is no /dev/dri: Mesa probes for a GPU driver, warns ("failed to create dri2
# screen") and falls back to CPU rendering. The d3d12 driver uses the GPU through WSLg instead.
ifeq ($(PLATFORM),linux)
  IS_WSL := $(shell grep -qi microsoft /proc/version 2>/dev/null && echo yes)
endif
RUN_ENV := $(if $(IS_WSL),GALLIUM_DRIVER=$${GALLIUM_DRIVER:-d3d12} ,)

.DEFAULT_GOAL := help
.PHONY: help setup run probe probe-sql jt400 helper build release package icons test test-core test-ui lint \
        fmt fmt-check deny verify doc clean outdated update upgrade \
        release-patch release-minor release-major

help:
	@echo "Parley - make targets ($(PLATFORM))"
	@echo ""
	@echo "  Getting started"
	@echo "    setup         Install system libraries, Rust toolchain and cargo tools (once per machine)"
	@echo "    run           Run the desktop app (HOST=name connects at once)"
	@echo "    probe         Connect to HOST without the UI, press KEYS, print each screen"
	@echo "    probe-sql     Ask HOST which SQL routes it offers, without signing on (R-NET-8)"
	@echo "    jt400         Fetch JTOpen (pinned, checked) for the SQL helper into target/helpers"
	@echo "    helper        Build and check the SQL helper (needs a JDK 11+)"
	@echo ""
	@echo "  Quality"
	@echo "    verify        Everything CI checks: fmt-check, lint, test, deny"
	@echo "    test          All tests (engine + UI)"
	@echo "    test-core     Engine tests only"
	@echo "    test-ui       UI tests only (no window needed)"
	@echo "    lint          Clippy with warnings as errors"
	@echo "    fmt           Format all code"
	@echo "    deny          Check dependency licences and security advisories"
	@echo ""
	@echo "  Releasing (docs/RELEASING.md)"
	@echo "    release-patch Bump x.y.Z, verify, commit, tag and push: starts the Release workflow"
	@echo "    release-minor Same for x.Y.0"
	@echo "    release-major Same for X.0.0"
	@echo ""
	@echo "  Dependencies"
	@echo "    outdated      List dependencies with newer versions, compatible and breaking"
	@echo "    update        Update Cargo.lock within the versions Cargo.toml allows, then verify"
	@echo "    upgrade       Bump Cargo.toml to the latest versions, breaking ones too, then verify"
	@echo "                  (needs cargo-edit; upgrade egui, eframe and egui_kittest together)"
	@echo ""
	@echo "  Building"
	@echo "    build         Debug build of the desktop app"
	@echo "    release       Optimised build of the desktop app"
	@echo "    package       Installers for this OS in target/packages (needs cargo-packager)"
	@echo "    icons         Render the app icon from crates/parley-app/icons/parley.svg (needs resvg)"
	@echo "    doc           Build and open the API docs"
	@echo "    clean         Remove build output"

# ---------------------------------------------------------------- setup

setup: setup-$(PLATFORM)

.PHONY: setup-linux setup-macos setup-windows
setup-linux:
	./scripts/setup-linux.sh
setup-macos:
	./scripts/setup-macos.sh
setup-windows:
	powershell -ExecutionPolicy Bypass -File scripts\setup-windows.ps1

# ---------------------------------------------------------------- run

run:
	$(RUN_ENV)cargo run -p $(APP) -- $(HOST)

# make probe HOST=pub400.com [KEYS=help,reset,enter] [ARGS="--tls --ccsid 277"]
probe:
	cargo run -p $(CORE) --example probe -- $(HOST) $(ARGS) $(if $(KEYS),--keys $(KEYS),)

# JTOpen for the SQL helper, pinned and checked (R-SQL-13, D20)
HELPERS := target/helpers
jt400:
	scripts/fetch-jt400.sh $(HELPERS)

# The SQL helper (R-SQL-11, D20): needs a JDK 11 or later (and bash: Git Bash on Windows). Builds
# it into target/helpers, runs its self-test, then the Rust tests that start it.
helper:
	bash scripts/build-helper.sh $(HELPERS)
	cargo test -q -p $(CORE) sql:: -- --ignored
	cargo test -q -p $(UI) sql -- --ignored

# make probe-sql HOST=myibmi.example.com: which SQL routes the host offers, without signing on (R-NET-8)
# make probe-sql HOST=... ARGS="--sign-on USER [--tls] [--ccsid 277]": one recorded sign-on (R-NET-9)
probe-sql:
	cargo run -q -p $(CORE) --example sqlprobe -- $(HOST) $(ARGS)

# ---------------------------------------------------------------- build

build:
	cargo build -p $(APP)

release:
	cargo build -p $(APP) --release

# Installers (R-NF-1): .deb and .AppImage on Linux, .msi on Windows, .dmg on macOS.
PACKAGE_FORMATS_linux := deb,appimage
PACKAGE_FORMATS_windows := wix
PACKAGE_FORMATS_macos := dmg

package: release helper
	cargo packager -p $(APP) --release -f $(PACKAGE_FORMATS_$(PLATFORM))

icons:
	python3 scripts/render-icons.py

doc:
	cargo doc --workspace --no-deps --open

clean:
	cargo clean

# ---------------------------------------------------------------- quality

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

test:
	cargo test --workspace

test-core:
	cargo test -p $(CORE)

test-ui:
	cargo test -p $(UI)

lint:
	cargo clippy --workspace --all-targets -- -D warnings

deny:
	cargo deny check

verify: fmt-check lint test deny helper

# ---------------------------------------------------------------- releasing

# A clean, up-to-date main only; asks before it changes anything (scripts/release.sh).
release-patch release-minor release-major:
	bash scripts/release.sh $(subst release-,,$@)

# ---------------------------------------------------------------- dependencies

outdated:
	cargo update --dry-run --verbose

update:
	cargo update
	$(MAKE) verify

upgrade:
	cargo upgrade --incompatible
	cargo update
	$(MAKE) verify
