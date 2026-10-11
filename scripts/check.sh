#!/usr/bin/env bash
# Local CI for AXS: Rust checks, crate packaging, the TypeScript SDK, and the VS Code extension.
#
# Usage: scripts/check.sh [--skip-sdk] [--skip-extension] [--vscode-smoke]
# Requires: cargo (with rustfmt and clippy); node and npm unless both are skipped;
# for the SDK, the wasm32-unknown-unknown target and the locked wasm-bindgen CLI.
# The extension bundles the SDK built in packages/axs. --vscode-smoke also
# installs the VSIX into a local VS Code and runs the in-editor smoke test
# (opens a VS Code window).
# Exits with the status of the first failing step.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

skip_extension=0
skip_sdk=0
vscode_smoke=0
for argument in "$@"; do
    case "$argument" in
        --skip-extension) skip_extension=1 ;;
        --skip-sdk) skip_sdk=1 ;;
        --vscode-smoke) vscode_smoke=1 ;;
        -h | --help)
            sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "error: unknown argument: $argument" >&2
            exit 2
            ;;
    esac
done

# rustup installs into ~/.cargo/bin, which non-login shells may not have on PATH.
if ! command -v cargo > /dev/null 2>&1 && [ -x "${CARGO_HOME:-$HOME/.cargo}/bin/cargo" ]; then
    PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
fi

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    bold=$'\033[1m' green=$'\033[32m' red=$'\033[31m' reset=$'\033[0m'
else
    bold='' green='' red='' reset=''
fi

current=''
started=$SECONDS

on_exit() {
    local status=$?
    if [ "$status" -ne 0 ] && [ -n "$current" ]; then
        printf '\n%sFAILED%s: %s (exit %d)\n' "$red$bold" "$reset" "$current" "$status" >&2
    fi
}
trap on_exit EXIT

require() {
    if ! command -v "$1" > /dev/null 2>&1; then
        echo "error: required command not found: $1" >&2
        exit 127
    fi
}

step() {
    current="$1"
    shift
    printf '\n%s==> %s%s\n' "$bold" "$current" "$reset"
    printf '    $ %s\n' "$*"
    "$@"
    printf '%sok%s: %s\n' "$green" "$reset" "$current"
}

require cargo
printf 'Rust: %s\n' "$(rustc --version)"

step "Rust formatting" cargo fmt --all --check
step "Clippy" cargo clippy --locked --workspace --all-targets -- -D warnings
step "Tests" cargo test --locked --workspace
step "Doctests" cargo test --locked --doc
step "Crate package" cargo package --locked --allow-dirty -p axs

if [ "$skip_sdk" -eq 1 ]; then
    printf '\nSkipping TypeScript SDK (--skip-sdk).\n'
else
    require node
    require npm
    sdk="$root/packages/axs"
    step "WASM bindings Clippy (wasm32)" cargo clippy --locked -p axs-wasm --target wasm32-unknown-unknown -- -D warnings
    step "SDK dependencies" npm --prefix "$sdk" ci --no-audit --no-fund
    step "SDK build (Rust to WebAssembly to TypeScript)" npm --prefix "$sdk" run build
    step "SDK typecheck" npm --prefix "$sdk" run typecheck
    step "SDK tests" npm --prefix "$sdk" test
    step "SDK package install test" npm --prefix "$sdk" run test:package
fi

if [ "$skip_extension" -eq 1 ]; then
    printf '\nSkipping VS Code extension (--skip-extension).\n'
else
    require node
    require npm
    printf '\nNode: %s, npm: %s\n' "$(node --version)" "$(npm --version)"
    extension="$root/editors/vscode"
    step "Extension dependencies" npm --prefix "$extension" ci --no-audit --no-fund
    step "Extension typecheck" npm --prefix "$extension" run typecheck
    step "Extension tests (Smart Enter, positions, grammar, WebAssembly runtime)" npm --prefix "$extension" test
    step "Extension VSIX package" npm --prefix "$extension" run package
    current="Extension VSIX contents"
    vsix="$(ls "$extension"/*.vsix | head -n 1)"
    contents="$(unzip -Z1 "$vsix" | sort)"
    expected="$(printf '%s\n' '[Content_Types].xml' extension.vsixmanifest extension/LICENSE.txt \
        extension/changelog.md extension/dist/axs_wasm_bg.wasm extension/dist/extension.js \
        extension/language-configuration.json extension/package.json extension/readme.md \
        extension/syntaxes/axs.tmLanguage.json | sort)"
    if [ "$contents" != "$expected" ]; then
        printf 'error: unexpected VSIX contents:\n%s\n' "$contents" >&2
        exit 1
    fi
    printf '%sok%s: %s (%s)\n' "$green" "$reset" "$current" "$(basename "$vsix")"
    if [ "$vscode_smoke" -eq 1 ]; then
        step "Installed-extension smoke test in VS Code" node "$extension/scripts/smoke-test.mjs" --vsix "$vsix"
    fi
fi

current=''
printf '\n%sAll checks passed%s in %ds.\n' "$green$bold" "$reset" "$((SECONDS - started))"
