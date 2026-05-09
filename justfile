wasm := "target/wasm32-wasip2/release/openapi_bridge.wasm"

act := env("ACT", "npx @actcore/act")
actbuild := env("ACT_BUILD", "npx @actcore/act-build")
hurl := env("HURL", "npx @orangeopensource/hurl")
registry := env("OCI_REGISTRY", "ghcr.io/actpkg")
port := `npx get-port-cli`
addr := "[::1]:" + port
baseurl := "http://" + addr
petstore_spec := env("PETSTORE_SPEC", "https://petstore3.swagger.io/api/v3/openapi.json")

init:
    wit-deps

setup: init
    prek install

build:
    cargo build --release
    {{actbuild}} pack {{wasm}}

test:
    #!/usr/bin/env bash
    set -euo pipefail
    {{act}} run {{wasm}} --http --listen "{{addr}}" --http-policy open &
    trap "kill $!" EXIT
    npx wait-on -t 180s {{baseurl}}/info
    {{hurl}} --test \
      --variable "baseurl={{baseurl}}" \
      --variable "petstore_spec={{petstore_spec}}" \
      e2e/*.hurl

publish:
    #!/usr/bin/env bash
    set -euo pipefail
    INFO=$({{act}} info {{wasm}} --format json)
    NAME=$(echo "$INFO" | jq -r .name)
    VERSION=$(echo "$INFO" | jq -r .version)
    SOURCE=$(git remote get-url origin 2>/dev/null | sed 's/\.git$//' | sed 's|git@github.com:|https://github.com/|' || echo "")
    OUTPUT=$({{actbuild}} push {{wasm}} "{{registry}}/$NAME:$VERSION" \
      --skip-if-identical \
      --also-tag latest \
      --source "$SOURCE" 2>&1)
    echo "$OUTPUT"
    DIGEST=$(echo "$OUTPUT" | grep "^Digest:" | awk '{print $2}')
    if [ -n "${GITHUB_OUTPUT:-}" ]; then
      echo "image={{registry}}/$NAME" >> "$GITHUB_OUTPUT"
      echo "digest=$DIGEST" >> "$GITHUB_OUTPUT"
    fi
