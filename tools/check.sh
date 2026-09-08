#!/usr/bin/env bash
# One-command project verification: format, lint, test, scene regeneration
# drift. Run before considering any change done.
set -euo pipefail
cd "$(dirname "$0")/.."

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-F:/diff/.cargo-target}"

echo "== fmt =="
cargo fmt --check

echo "== clippy =="
cargo clippy --all-targets -- -D warnings

echo "== test =="
cargo test

echo "== scene generators in sync =="
# Regenerating must reproduce the on-disk JSON exactly; otherwise someone
# hand-edited the JSON (will be lost on next regen) or the generator changed
# without regenerating.
for scene in dust2 showcase; do
    cp "assets/scenes/$scene.json" "/tmp/$scene.before.json"
done
python tools/gen_dust2.py >/dev/null
python tools/gen_showcase.py >/dev/null
for scene in dust2 showcase; do
    if ! cmp -s "assets/scenes/$scene.json" "/tmp/$scene.before.json"; then
        echo "ERROR: $scene.json does not match its generator output" >&2
        exit 1
    fi
    rm -f "/tmp/$scene.before.json"
done

echo "ALL CHECKS PASSED"
