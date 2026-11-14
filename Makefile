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
.PHONY: help setup run run-sql probe probe-sql jt400 helper llama build release package icons test test-core test-ui lint \
        fmt fmt-check deny verify doc clean outdated update upgrade \
        release-patch release-minor release-major

help:
	@echo "Parley - make targets ($(PLATFORM))"
	@echo ""
	@echo "  Getting started"
	@echo "    setup         Install system libraries, Rust toolchain and cargo tools (once per machine)"
	@echo "    run           Run the desktop app (HOST=name connects at once)"
	@echo "    run-sql       Run Parley SQL, the SQL window on its own (CONNECTION=name signs on at once)"
	@echo "    probe         Connect to HOST without the UI, press KEYS, print each screen"
	@echo "    probe-sql     Ask HOST which SQL routes it offers, without signing on (R-NET-8)"
	@echo "    jt400         Fetch JTOpen (pinned, checked) for the SQL helper into target/helpers"
	@echo "    helper        Build and check the SQL helper (needs a JDK 11+)"
	@echo "    llama         Serve a local model with llama.cpp on 127.0.0.1:LLAMA_PORT (apt/brew/winget install llama.cpp)"
	@echo "                  LLAMA_MODEL=repo:quant (Hugging Face) or a .gguf file; LLAMA_ARGS= more flags"
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

# Builds Parley SQL too: Tools > SQL... starts it from beside Parley (R-SQL-70)
run:
	cargo build -p $(APP) --bins
	$(RUN_ENV)cargo run -p $(APP) -- $(HOST)

# Parley SQL, the SQL window on its own (R-SQL-45); CONNECTION= opens a saved connection's sign-on
run-sql:
	$(RUN_ENV)cargo run -p $(APP) --bin parley-sql -- $(CONNECTION)

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

# A local model behind llama.cpp's OpenAI-compatible server, for trying models out. Needs llama-server
# on PATH: sudo apt install llama.cpp (recent Ubuntu and Debian), brew install llama.cpp, winget install
# llama.cpp, or a release from github.com/ggml-org/llama.cpp.
# LLAMA_MODEL is a Hugging Face repo:quant, downloaded once into llama.cpp's cache, or a local .gguf file.
# The default is a 7B coder model at 4 bits (~4.7 GB): it runs on a CPU with 16 GB of memory.
# --jinja applies the model's own chat template, which tool calls need.
LLAMA_MODEL ?= bartowski/Qwen2.5-Coder-7B-Instruct-GGUF:Q4_K_M
LLAMA_PORT ?= 8080
LLAMA_CTX ?= 16384

# Is llama-server on PATH? Expanded only when `make llama` runs. `where` prints the full path on
# Windows (in cmd and Git Bash alike; paths with spaces split into words, so match the last one).
ifeq ($(PLATFORM),windows)
  LLAMA_FOUND = $(filter %llama-server.exe,$(shell where llama-server 2>&1))
else
  LLAMA_FOUND = $(shell command -v llama-server 2>/dev/null)
endif
LLAMA_INSTALL_linux := sudo apt install llama.cpp   (Ubuntu and Debian; or: brew install llama.cpp)
LLAMA_INSTALL_macos := brew install llama.cpp
LLAMA_INSTALL_windows := winget install llama.cpp   (then open a new terminal)
define LLAMA_MISSING
llama-server (llama.cpp) is not installed or not on PATH. Install it with:

    $(LLAMA_INSTALL_$(PLATFORM))

or unpack a release from https://github.com/ggml-org/llama.cpp/releases and put its folder on PATH.
endef

llama:
	$(if $(LLAMA_FOUND),,$(info $(LLAMA_MISSING))$(error llama-server not found))
	llama-server $(if $(filter %.gguf,$(LLAMA_MODEL)),-m,-hf) $(LLAMA_MODEL) --host 127.0.0.1 --port $(LLAMA_PORT) -c $(LLAMA_CTX) --jinja $(LLAMA_ARGS)

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
