# 🛡️ Password Vault (Local • Zero-Knowledge • Tauri + Rust + Firefox/Zen Extension)

Cofre de senhas local estilo Bitwarden/KeePassXC construído com **Tauri v2 (Rust)**, **React + TypeScript + TailwindCSS**, **SQLite** e integração automática com **Firefox / Zen Browser** via **Native Messaging API + Unix Domain Socket**.

## Arquitetura

```text
password-vault/
├── crates/
│   ├── vault-core/          # Core criptográfico (Argon2id, XChaCha20-Poly1305, Zeroize), SQLite e Protocolo IPC
│   └── vault-native-host/   # Binário Native Messaging (stdin/stdout <-> Unix Socket) para Firefox/Zen
├── src-tauri/               # App Desktop Tauri (Estado do cofre em RAM + Servidor Unix Socket 0600)
├── src/                     # Interface Desktop (React + TypeScript + TailwindCSS)
├── browser-extension/       # Extensão WebExtension (Manifest V2/Gecko) para Auto-fill e Auto-save
└── scripts/
    └── install-native-host.sh # Registra o host nativo em ~/.mozilla/native-messaging-hosts/
```

### Fluxo de Segurança e Criptografia
1. **Derivação de Chave (KDF):** A Senha Mestra passa pelo **Argon2id** (64 MiB RAM, 3 iterações, 4 lanes) com um salt aleatório de 128 bits para derivar a `Master Key`.
2. **Envelope Encryption:** A `Master Key` descriptografa a `Vault Key` (256 bits), que permanece na memória RAM dentro de uma estrutura protegida por `ZeroizeOnDrop`. Ao bloquear o cofre, os bytes da chave na RAM são sobrescritos com zeros imediatamente.
3. **Criptografia Autenticada (AEAD):** Cada credencial e nota é criptografada com **XChaCha20-Poly1305** (nonce de 192 bits).
4. **Comunicação Isolada com o Navegador (Opção B):**
   - Nenhuma porta TCP/HTTP fica aberta em `localhost`.
   - O App Desktop cria um **Unix Domain Socket** em `$XDG_RUNTIME_DIR/password-vault.sock` com permissão `0600` (exclusivo do seu usuário Linux).
   - O Firefox / Zen Browser inicia o binário `vault-native-host`, que conversa via `stdin/stdout` com a extensão (`password-vault@local.dev`) e repassa os comandos para o Unix Socket.

---

## Como Executar

### 1. Registrar o Native Messaging Host (Firefox / Zen Browser)
Execute o script que compila o binário `vault-native-host` em modo release e cria o manifesto em `~/.mozilla/native-messaging-hosts/com.local.password_vault.json`:

```bash
./scripts/install-native-host.sh
```

### 2. Instalar e Rodar o App Desktop (Tauri)
```bash
npm install
npm run dev
# Em outro terminal (ou via cargo tauri dev):
cargo run -p password-vault-desktop
```

### 3. Carregar a Extensão no Firefox / Zen Browser
1. Abra o Firefox ou Zen Browser e digite na barra de endereços: `about:debugging#/runtime/this-firefox`
2. Clique em **"Carregar extensão temporária..."** (*Load Temporary Add-on...*).
3. Selecione o arquivo `browser-extension/manifest.json`.
4. Agora, ao abrir qualquer tela de login no navegador:
   - **Auto-fill:** Ao focar no campo de senha, o popover do Password Vault aparece com os logins daquele domínio.
   - **Auto-save:** Ao enviar um formulário de login com uma nova credencial, aparece o banner no canto superior direito perguntando se deseja criptografar e salvar no cofre.
