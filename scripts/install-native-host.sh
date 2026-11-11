#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOST_NAME="com.local.password_vault"
EXTENSION_ID="password-vault@local.dev"

echo "==> Compilando binários vault-native-host e password-vault-desktop (Rust Release)..."
cargo build --manifest-path "$PROJECT_ROOT/Cargo.toml" --release --workspace

BINARY_PATH="$PROJECT_ROOT/target/release/vault-native-host"

if [[ ! -f "$BINARY_PATH" ]]; then
  echo "Erro: Binário não encontrado em $BINARY_PATH"
  exit 1
fi

write_manifest() {
  local target_dir="$1"
  mkdir -p "$target_dir"
  local manifest_file="$target_dir/${HOST_NAME}.json"
  cat > "$manifest_file" <<EOF
{
  "name": "${HOST_NAME}",
  "description": "Local Password Vault Native Messaging Host (stdin/stdout <-> Unix Socket)",
  "path": "${BINARY_PATH}",
  "type": "stdio",
  "allowed_extensions": [
    "${EXTENSION_ID}"
  ]
}
EOF
  echo "✅ Manifesto instalado em: $manifest_file"
}

# 1. Firefox / Zen Browser nativo
write_manifest "$HOME/.mozilla/native-messaging-hosts"

# 2. Firefox Flatpak (org.mozilla.firefox)
write_manifest "$HOME/.var/app/org.mozilla.firefox/.mozilla/native-messaging-hosts"

# 3. Zen Browser Flatpak (app.zen_browser.zen)
write_manifest "$HOME/.var/app/app.zen_browser.zen/.zen/native-messaging-hosts"
write_manifest "$HOME/.var/app/app.zen_browser.zen/.mozilla/native-messaging-hosts"

# Se o Firefox Flatpak estiver instalado, garante permissão de leitura ao diretório do socket e binário
if command -v flatpak >/dev/null 2>&1; then
  flatpak override --user org.mozilla.firefox --filesystem="$HOME/.local/share/password-vault:create" 2>/dev/null || true
  flatpak override --user org.mozilla.firefox --filesystem="$PROJECT_ROOT/target/release:ro" 2>/dev/null || true
  flatpak override --user org.mozilla.firefox --talk-name=org.freedesktop.Flatpak 2>/dev/null || true
fi
