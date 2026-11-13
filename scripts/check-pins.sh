#!/usr/bin/env bash
# Looks for what is pinned and has a newer version, and for pins that disagree between files. Prints a Markdown report, nothing
# when all is current; always exits 0, the weekly workflow (.github/workflows/pins.yml) decides what to do with the report.
#
# Everything here is pinned on purpose (see AGENTS.md); Dependabot covers the crates in Cargo.lock, the actions and the base image,
# and ignores wasm-bindgen and topcoat on purpose, so this looks at what it does not: the toolchain, the tools installed by hand,
# and the npm package of the browser job. A bump is made on purpose, in a commit of its own.
set -euo pipefail
cd "$(dirname "$0")/.."

ua='tapiti-pins-check'
crate_latest() { curl -fsSL -H "User-Agent: $ua" "https://crates.io/api/v1/crates/$1" | jq -r '.crate.max_stable_version'; }
rust_latest() { # the whole manifest first: awk stops at the first match, and curl would complain of a closed pipe
  local manifest
  manifest=$(curl -fsSL https://static.rust-lang.org/dist/channel-rust-stable.toml)
  awk '/^\[pkg.rust\]/{f=1;next} f&&/^version = /{gsub(/"/,"",$3); print $3; exit}' <<<"$manifest"
}
behind=() mismatch=()
check() { # name, where, pinned, latest
  [ "$3" = "$4" ] || behind+=("| $1 | $2 | $3 | $4 |")
}
same() { # name, the values found
  [ "$(printf '%s\n' "$2" | wc -l)" -le 1 ] || mismatch+=("- $1: $(printf '%s' "$2" | tr '\n' ' ')")
}

rust=$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)
ci_rust=$(grep -h 'toolchain: ' .github/workflows/*.yml | sed 's/.*toolchain: *//' | sort -u)
image_rust=$(sed -n 's/^FROM rust:\([0-9.]*\)-.*/\1/p' Dockerfile | sort -u)
same "the Rust toolchain (rust-toolchain.toml, the workflows, the Dockerfile)" "$(printf '%s\n%s\n%s\n' "$rust" "$ci_rust" "$image_rust" | sort -u)"
check "Rust" "rust-toolchain.toml, workflows, Dockerfile" "$rust" "$(rust_latest)"

bindgen=$(sed -n 's/^wasm-bindgen = "=\(.*\)"/\1/p' crates/tapiti-wasm/Cargo.toml)
bindgen_cli=$(grep -hEo 'wasm-bindgen-cli --version [0-9.]+' .github/workflows/*.yml Dockerfile | awk '{print $3}' | sort -u)
same "wasm-bindgen (the crate and the CLI)" "$(printf '%s\n%s\n' "$bindgen" "$bindgen_cli" | sort -u)"
check "wasm-bindgen" "tapiti-wasm, workflows, Dockerfile" "$bindgen" "$(crate_latest wasm-bindgen)"

topcoat=$(sed -n 's/^topcoat = { version = "=\([0-9.]*\)".*/\1/p' apps/site/Cargo.toml)
topcoat_cli=$(grep -hEo 'topcoat-cli --version [0-9.]+' .github/workflows/*.yml Dockerfile | awk '{print $3}' | sort -u)
same "topcoat (the crate and the CLI)" "$(printf '%s\n%s\n' "$topcoat" "$topcoat_cli" | sort -u)"
check "topcoat" "apps/site, workflows, Dockerfile" "$topcoat" "$(crate_latest topcoat)"

chef=$(grep -hEo 'cargo-chef --version [0-9.]+' Dockerfile | awk '{print $3}' | sort -u)
check "cargo-chef" "Dockerfile" "$chef" "$(crate_latest cargo-chef)"

deb=$(grep -hEo 'cargo-deb --version [0-9.]+' .github/workflows/*.yml | awk '{print $3}' | sort -u)
check "cargo-deb" "release.yml" "$deb" "$(crate_latest cargo-deb)"
rpm=$(grep -hEo 'cargo-generate-rpm --version [0-9.]+' .github/workflows/*.yml | awk '{print $3}' | sort -u)
check "cargo-generate-rpm" "release.yml" "$rpm" "$(crate_latest cargo-generate-rpm)"

axe=$(grep -hEo 'axe-core@[0-9.]+' .github/workflows/*.yml | sort -u | cut -d@ -f2)
check "axe-core" "ci.yml (browser job)" "$axe" "$(npm view axe-core version)"

if [ ${#behind[@]} -gt 0 ] || [ ${#mismatch[@]} -gt 0 ]; then
  echo "A pinned version is behind, or pins that must agree do not. Read the release notes and the advisories, then bump on purpose, in a commit of its own."
  if [ ${#behind[@]} -gt 0 ]; then
    printf '\n| What | Where | Pinned | Latest |\n|---|---|---|---|\n'
    printf '%s\n' "${behind[@]}"
  fi
  if [ ${#mismatch[@]} -gt 0 ]; then
    printf '\nPins that disagree:\n\n'
    printf '%s\n' "${mismatch[@]}"
  fi
fi
