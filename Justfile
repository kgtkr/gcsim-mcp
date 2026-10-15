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
    cd wasm-engine && go get -u github.com/genshinsim/gcsim@main && go mod tidy
    sed -i 's|vendorHash = ".*";|vendorHash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";|' wasm-engine/default.nix
    NEW_HASH=$(nix build .#wasm-engine 2>&1 | grep -o 'got: *sha256-[^ ]*' | awk '{print $2}' || true); \
    if [ -n "$NEW_HASH" ]; then \
        sed -i "s|vendorHash = \".*\";|vendorHash = \"$NEW_HASH\";|" wasm-engine/default.nix; \
    fi
