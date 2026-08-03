SHELL := /bin/bash

DIST_DIR ?= dist
CLEAN_DIST ?= 1
LOCAL_BUILD_ENV ?= .env.build.local

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
	@printf '%s\n' 'Nauterm build targets'
	@printf '%s\n' ''
	@printf '%s\n' 'Checks:'
	@printf '%s\n' '  make analyze                 Run Dart analyzer'
	@printf '%s\n' '  make test-data               Run Flutter data-store test'
	@printf '%s\n' '  make test-native-database    Run Rust database tests'
	@printf '%s\n' '  make verify-icons            Verify generated platform app icons'
	@printf '%s\n' '  make generate-rust-licenses Regenerate bundled Rust license notices'
	@printf '%s\n' ''
	@printf '%s\n' 'Development:'
	@printf '%s\n' '  make run                     Prepare icons, then run Flutter'
	@printf '%s\n' '  make prepare-icons           Copy generated icons into platform projects'
	@printf '%s\n' ''
	@printf '%s\n' 'Native library:'
	@printf '%s\n' '  make native-debug            Build native Rust library for debug'
	@printf '%s\n' '  make native-release          Build native Rust library for release'
	@printf '%s\n' ''
	@printf '%s\n' 'Packages:'
	@printf '%s\n' '  make package-current         Build package for the current OS/arch'
	@printf '%s\n' '  make package-macos-arm64     Build macOS .app.zip and .dmg for arm64 runner'
	@printf '%s\n' '  make package-macos-x86_64    Build macOS .app.zip and .dmg for x86_64 runner'
	@printf '%s\n' '  make package-linux-x86_64    Build Linux AppImage, deb, rpm for x86_64'
	@printf '%s\n' '  make package-linux-arm64     Build Linux AppImage, deb, rpm for arm64 runner'
	@printf '%s\n' '  make package-windows-x86_64  Build Windows x86_64 zip
  make package-windows-arm64   Build Windows arm64 zip'
	@printf '%s\n' ''
	@printf '%s\n' 'Variables:'
	@printf '%s\n' '  DIST_DIR=dist                Package output directory'
	@printf '%s\n' '  CLEAN_DIST=1                 Clean DIST_DIR before single package target'
	@printf '%s\n' '  LOCAL_BUILD_ENV=.env.build.local'
	@printf '%s\n' '  NAUTERM_BUILD_NUMBER=1       Override the application build number'
	@printf '%s\n' ''
	@printf '%s\n' 'Note: arch package targets should run on a matching host/runner unless the'
	@printf '%s\n' '      platform toolchain explicitly supports cross-compilation.'

.PHONY: clean-dist
clean-dist:
	@case "$(DIST_DIR)" in \
		""|"."|".."|/*|../*|*/..|*/../*) \
			echo "DIST_DIR must be a relative child of the project directory: $(DIST_DIR)" >&2; \
			exit 1 ;; \
	esac
	rm -rf "$(DIST_DIR)"
	mkdir -p "$(DIST_DIR)"

.PHONY: analyze
analyze:
	dart analyze

.PHONY: run
run: prepare-icons
	@bash scripts/run_flutter.sh

.PHONY: test-data
test-data:
	flutter test test/nauterm_data_store_test.dart

.PHONY: test-native-database
test-native-database:
	cargo test --manifest-path native/nauterm_ffi/Cargo.toml --features test-credential-store database::tests

.PHONY: native-debug
native-debug:
	bash scripts/build_native.sh debug

.PHONY: native-release
native-release:
	bash scripts/build_native.sh release

.PHONY: generate-icons
generate-icons:
	bash scripts/generate_app_icons.sh

.PHONY: prepare-icons
prepare-icons:
	bash scripts/prepare_app_icons.sh

.PHONY: verify-icons
verify-icons:
	bash scripts/ci/verify_app_icons.sh

.PHONY: generate-rust-licenses
generate-rust-licenses:
	bash scripts/generate_rust_licenses.sh

.PHONY: package-current
package-current:
	@case "$$(uname -s)" in \
		Darwin) \
			arch="$$(uname -m)"; \
			if [ "$$arch" = "arm64" ]; then \
				$(MAKE) package-macos-arm64; \
			else \
				$(MAKE) package-macos-x86_64; \
			fi ;; \
		Linux) \
			arch="$$(uname -m)"; \
			if [ "$$arch" = "aarch64" ] || [ "$$arch" = "arm64" ]; then \
				$(MAKE) package-linux-arm64; \
			else \
				$(MAKE) package-linux-x86_64; \
			fi ;; \
		MINGW*|MSYS*|CYGWIN*) \
			$(MAKE) package-windows-x86_64 ;; \
		*) \
			echo "Unsupported OS: $$(uname -s)" >&2; exit 1 ;; \
	esac

.PHONY: package-macos-arm64
package-macos-arm64: package-macos-arm64-bundle

.PHONY: package-macos-arm64-bundle
package-macos-arm64-bundle:
	DIST_DIR="$(DIST_DIR)" CLEAN_DIST="$(CLEAN_DIST)" bash scripts/ci/package_macos.sh arm64

.PHONY: package-macos-x86_64
package-macos-x86_64: package-macos-x86_64-bundle

.PHONY: package-macos-x86_64-bundle
package-macos-x86_64-bundle:
	DIST_DIR="$(DIST_DIR)" CLEAN_DIST="$(CLEAN_DIST)" bash scripts/ci/package_macos.sh x86_64

.PHONY: package-linux-x86_64
package-linux-x86_64: package-linux-x86_64-bundle

.PHONY: package-linux-x86_64-bundle
package-linux-x86_64-bundle:
	DIST_DIR="$(DIST_DIR)" CLEAN_DIST="$(CLEAN_DIST)" bash scripts/ci/package_linux.sh x86_64 amd64 x86_64 x86_64

.PHONY: package-linux-arm64
package-linux-arm64: package-linux-arm64-bundle

.PHONY: package-linux-arm64-bundle
package-linux-arm64-bundle:
	DIST_DIR="$(DIST_DIR)" CLEAN_DIST="$(CLEAN_DIST)" bash scripts/ci/package_linux.sh arm64 arm64 aarch64 aarch64

.PHONY: package-windows package-windows-x86_64 package-windows-arm64
package-windows: package-windows-x86_64

package-windows-x86_64:
	DIST_DIR="$(DIST_DIR)" CLEAN_DIST="$(CLEAN_DIST)" pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/ci/package_windows.ps1 -Arch x86_64

package-windows-arm64:
	DIST_DIR="$(DIST_DIR)" CLEAN_DIST="$(CLEAN_DIST)" pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/ci/package_windows.ps1 -Arch arm64
