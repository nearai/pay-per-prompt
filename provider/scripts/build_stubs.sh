#!/usr/bin/env bash
set -euo pipefail

# remove old generated stubs
sudo rm -rf stubs/
mkdir -p stubs/openaiapi stubs/openaiclient
touch stubs/openaiapi/.gitkeep
touch stubs/openaiclient/.gitkeep

# Generate api stubs
sudo openapi-generator-cli generate \
    -i ./configs/openai_openapi.yaml \
    -g rust-axum \
    --skip-validate-spec \
    --package-name openaiapi \
    -o ./stubs/openaiapi
sudo chown -R $(whoami) stubs/openaiapi

# Generate client stubs
sudo openapi-generator-cli generate \
    -i ./configs/openai_openapi.yaml \
    -g rust \
    --skip-validate-spec \
    --package-name openaiclient \
    -o ./stubs/openaiclient
sudo chown -R $(whoami) stubs/openaiclient

pushd stubs/openaiapi
cargo fmt --all
popd

pushd stubs/openaiclient
cargo fmt --all
popd
