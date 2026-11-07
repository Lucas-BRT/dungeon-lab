#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

log_file="${1:-target/generator-bench-$(date +%Y%m%d-%H%M%S).log}"
mkdir -p "$(dirname -- "$log_file")"

printf 'Bench log: %s\n' "$log_file"
{
    printf '=== Environment ===\n'
    printf 'Date: %s\n' "$(date --iso-8601=seconds)"
    printf 'Branch: %s\n' "$(git branch --show-current)"
    printf 'Host: %s\n' "$(uname -a)"
    printf 'Rust: %s\n' "$(rustc --version)"
    printf 'Cargo: %s\n' "$(cargo --version)"
    if command -v lscpu >/dev/null 2>&1; then
        lscpu | awk -F: '/^(Architecture|CPU\(s\)|Model name):/ { sub(/^[[:space:]]+/, "", $2); print $1 ": " $2 }'
    fi

    printf '\n=== CPU: full generator Criterion suite ===\n'
    cargo bench --offline -p dungeon-lab-core --bench generator

    printf '\n=== Vulkan: proximity GPU comparison ===\n'
    cargo run --offline --release -p dungeon-lab-core --example gpu_proximity

    printf '\n=== CPU: full generator phase profile at larger grids ===\n'
    cargo run --offline --release -p dungeon-lab-core --features diagnostic-profile --example profile_generator_scale
} 2>&1 | tee "$log_file"

printf '\nFinished. Send this file: %s\n' "$log_file"
