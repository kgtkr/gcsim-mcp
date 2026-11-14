default: dev

wasm:
    cd wasm-engine && GOOS=wasip1 GOARCH=wasm go build -trimpath -ldflags="-s -w" -o $WASM_PATH .

dev *args: wasm
    cd server && cargo run -- {{args}}

lint:
    cd wasm-engine && GOOS=wasip1 GOARCH=wasm go vet ./...
    cd wasm-engine && go mod verify
    cd server && cargo clippy --all --lib --tests -- -D warnings

test: wasm
    cd wasm-engine && GOOS=wasip1 GOARCH=wasm go test ./...
    cd server && cargo test

update-gcsim:
    #!/usr/bin/env bash
    set -euo pipefail
    LATEST_TAG=$(curl -fsSL https://api.github.com/repos/genshinsim/gcsim/releases/latest | grep -o '"tag_name": *"[^"]*"' | head -n1 | cut -d'"' -f4)
    COMMIT_SHA=$(git ls-remote https://github.com/genshinsim/gcsim.git "refs/tags/${LATEST_TAG}^{}" "refs/tags/${LATEST_TAG}" | tail -n1 | awk '{print $1}')
    echo "Updating gcsim to release ${LATEST_TAG} (${COMMIT_SHA})"
    cd wasm-engine && go get "github.com/genshinsim/gcsim@${COMMIT_SHA}" && go mod tidy
    cd ..
    sed -i 's|vendorHash = ".*";|vendorHash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";|' wasm-engine/default.nix
    NEW_HASH=$(nix build .#wasm-engine 2>&1 | grep -o 'got: *sha256-[^ ]*' | awk '{print $2}' || true)
    if [ -n "$NEW_HASH" ]; then
        sed -i "s|vendorHash = \".*\";|vendorHash = \"$NEW_HASH\";|" wasm-engine/default.nix
    fi
