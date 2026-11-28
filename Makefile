SHELL := /bin/bash

DIST_DIR ?= dist
CLEAN_DIST ?= 1
ARCH ?=
PROFILE ?= debug
CLIENT ?= all
LOCAL_BUILD_ENV ?= .env.build.local

ifeq ($(OS),Windows_NT)
rs_empty :=
rs_space := $(rs_empty) $(rs_empty)
rs_git_bash = $(if $(strip $(1)),$(wildcard $(subst $(rs_space),\$(rs_space),$(subst \,/,$(1))/$(2)/bin/bash.exe)))
GIT_BASH ?= $(or $(call rs_git_bash,$(ProgramW6432),Git),$(call rs_git_bash,$(ProgramFiles),Git),$(call rs_git_bash,$(LOCALAPPDATA),Programs/Git),bash)
BASH ?= $(GIT_BASH)
else
BASH ?= bash
endif

-include $(LOCAL_BUILD_ENV)

export NAUTERM_UPDATE_REPOSITORY
export NAUTERM_POSTHOG_API_KEY
export NAUTERM_POSTHOG_HOST
export NAUTERM_GITHUB_CLIENT_ID
export NAUTERM_GOOGLE_CLIENT_ID
export NAUTERM_GOOGLE_CLIENT_SECRET
export NAUTERM_ONEDRIVE_CLIENT_ID
export NAUTERM_DROPBOX_CLIENT_ID
export NAUTERM_BUILD_NUMBER
export SPARKLE_PUBLIC_ED_KEY
export SPARKLE_PRIVATE_KEY

.DEFAULT_GOAL := help

.PHONY: help
help:
	@printf '%s\n' 'Nauterm commands' '' 'Flutter'
	@printf '  %-27s %s\n' \
		'make flutter-run'          'Run the desktop client' \
		'make flutter-analyze'      'Run the Dart analyzer' \
		'make flutter-test-data'    'Run the Flutter data store tests' \
		'make flutter-build-native' 'Build the native libraries (PROFILE=debug or release)' \
		'make flutter-package'      'Build release packages for this platform' \
		'make flutter-appimage'     'Build only the AppImage on Linux' \
		'make flutter-appcast'      'Sign the macOS Sparkle update feed'
	@printf '\n%s\n' 'Nauterm RS'
	@printf '  %-27s %s\n' \
		'make rs-run'               'Run the desktop client' \
		'make rs-build'             'Build the debug executable' \
		'make rs-check'             'Check all Rust client targets' \
		'make rs-bundle-macos'      'Build a local debug .app bundle' \
		'make rs-package'           'Build release packages for this platform' \
		'make rs-appcast'           'Sign the macOS Sparkle update feed'
	@printf '\n%s\n' 'Shared tooling'
	@printf '  %-27s %s\n' \
		'make core-test-database'   'Run the shared Rust database tests' \
		'make icons-generate'       'Regenerate app icons on macOS' \
		'make icons-prepare'        'Copy generated icons into the platform projects' \
		'make icons-verify'         'Verify generated icons and platform copies' \
		'make licenses-generate'   'Regenerate Rust notices (CLIENT=all, flutter, or rs)' \
		'make dist-clean'           'Empty the package output directory'
	@printf '\n%s\n' 'Options'
	@printf '  %-27s %s\n' \
		'ARCH=arm64 or x86_64'     'Select the package or appcast architecture (default: host)' \
		'DIST_DIR=dist'            'Package and appcast output directory' \
		'CLEAN_DIST=0'             'Keep earlier packages in DIST_DIR when packaging' \
		'LOCAL_BUILD_ENV=path'     'Build settings file (default: .env.build.local)' \
		'NAUTERM_BUILD_NUMBER=1'   'Override the application build number' \
		'NAUTERM_DATA_DIR=path'    'Use a separate data directory for Flutter' \
		'NAUTERM_RS_DATA_DIR=path' 'Use a separate data directory for Nauterm RS'

# Flutter
.PHONY: flutter-run flutter-analyze flutter-test-data flutter-build-native flutter-appimage

flutter-run: icons-prepare
	@"$(BASH)" scripts/run_flutter.sh

flutter-analyze:
	dart analyze

flutter-test-data:
	flutter test test/nauterm_data_store_test.dart

flutter-build-native:
	@profile="$(PROFILE)"; \
	case "$$profile" in \
		debug|release) "$(BASH)" scripts/build_native.sh "$$profile" ;; \
		*) printf '%s\n' 'PROFILE must be debug or release.' >&2; exit 2 ;; \
	esac

flutter-appimage:
	DIST_DIR="$(DIST_DIR)" CLEAN_DIST="$(CLEAN_DIST)" NAUTERM_APPIMAGE_ONLY=1 \
		"$(BASH)" scripts/package_desktop.sh flutter "$(ARCH)"

# Nauterm RS
.PHONY: rs-run rs-build rs-check rs-bundle-macos

rs-run:
	"$(BASH)" nauterm-rs/scripts/run.sh

rs-build:
	"$(BASH)" nauterm-rs/scripts/build.sh

rs-check:
	cargo check -p nauterm-rs --all-targets --locked

rs-bundle-macos:
	"$(BASH)" nauterm-rs/scripts/bundle-macos.sh debug

# Packaging and Sparkle feeds for both clients
.PHONY: flutter-package rs-package flutter-appcast rs-appcast

flutter-package rs-package: %-package:
	DIST_DIR="$(DIST_DIR)" CLEAN_DIST="$(CLEAN_DIST)" \
		"$(BASH)" scripts/package_desktop.sh "$*" "$(ARCH)"

flutter-appcast rs-appcast: %-appcast:
	@arch="$(ARCH)"; \
	DIST_DIR="$(DIST_DIR)" "$(BASH)" scripts/ci/generate_sparkle_appcast.sh \
		"$${arch:-$$(uname -m)}" "$*"

# Shared tooling
.PHONY: core-test-database icons-generate icons-prepare icons-verify licenses-generate dist-clean

core-test-database:
	cargo test --manifest-path native/nauterm_core/Cargo.toml --features test-credential-store database::tests

icons-generate:
	"$(BASH)" scripts/generate_app_icons.sh

icons-prepare:
	"$(BASH)" scripts/prepare_app_icons.sh

icons-verify:
	"$(BASH)" scripts/ci/verify_app_icons.sh

licenses-generate:
	"$(BASH)" scripts/generate_rust_licenses.sh "$(CLIENT)"

dist-clean:
	@case "$(DIST_DIR)" in \
		""|"."|".."|/*|../*|*/..|*/../*) \
			echo "DIST_DIR must be a relative child of the project directory: $(DIST_DIR)" >&2; \
			exit 1 ;; \
	esac
	rm -rf -- "$(DIST_DIR)"
	mkdir -p -- "$(DIST_DIR)"
