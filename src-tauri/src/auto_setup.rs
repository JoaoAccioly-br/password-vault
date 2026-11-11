use std::fs;
use std::path::PathBuf;

const HOST_NAME: &str = "com.local.password_vault";
const EXTENSION_ID: &str = "password-vault@local.dev";

/// Configura automaticamente os manifestos do Native Messaging na máquina do usuário
/// toda vez que o App Desktop é iniciado (Zero-Config para quem baixar do GitHub).
pub fn ensure_native_messaging_installed() {
    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    let home_path = PathBuf::from(&home);

    // Localiza o binário vault-native-host (ao lado do executável principal ou em /usr/bin)
    let native_host_bin = resolve_native_host_binary(&home_path);

    // Copia o binário para ~/.local/share/password-vault/bin/vault-native-host
    // pois essa pasta é acessível tanto pelo sistema nativo quanto por navegadores em Flatpak
    let shared_bin_dir = home_path.join(".local/share/password-vault/bin");
    let _ = fs::create_dir_all(&shared_bin_dir);
    let target_bin = shared_bin_dir.join("vault-native-host");

    if let Some(src_bin) = native_host_bin {
        if src_bin != target_bin && src_bin.exists() {
            let _ = fs::copy(&src_bin, &target_bin);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = fs::metadata(&target_bin) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o755);
                    let _ = fs::set_permissions(&target_bin, perms);
                }
            }
        }
    }

    let final_bin_path = if target_bin.exists() {
        target_bin.to_string_lossy().to_string()
    } else {
        return;
    };

    let manifest_json = format!(
        r#"{{
  "name": "{HOST_NAME}",
  "description": "Local Password Vault Native Messaging Host",
  "path": "{final_bin_path}",
  "type": "stdio",
  "allowed_extensions": [
    "{EXTENSION_ID}"
  ]
}}"#
    );

    let host_dirs = [
        home_path.join(".mozilla/native-messaging-hosts"),
        home_path.join(".var/app/org.mozilla.firefox/.mozilla/native-messaging-hosts"),
        home_path.join(".var/app/app.zen_browser.zen/.zen/native-messaging-hosts"),
        home_path.join(".var/app/app.zen_browser.zen/.mozilla/native-messaging-hosts"),
    ];

    for dir in host_dirs {
        if fs::create_dir_all(&dir).is_ok() {
            let file_path = dir.join(format!("{HOST_NAME}.json"));
            let _ = fs::write(file_path, &manifest_json);
        }
    }
}

fn resolve_native_host_binary(home_path: &PathBuf) -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let sibling = parent.join("vault-native-host");
            if sibling.exists() {
                return Some(sibling);
            }
        }
    }

    let candidates = [
        home_path.join(".local/bin/vault-native-host"),
        PathBuf::from("/usr/local/bin/vault-native-host"),
        PathBuf::from("/usr/bin/vault-native-host"),
    ];

    candidates.into_iter().find(|p| p.exists())
}
