const NATIVE_HOST_NAME = "com.local.password_vault";
const BRIDGE_FALLBACK_URL = "http://127.0.0.1:49215/ipc";

/**
 * Tenta enviar via Native Messaging (stdin/stdout -> Unix Socket) com timeout de 1.2s.
 * Se o navegador estiver em sandbox restrito (Flatpak/Snap sem portal D-Bus configurado),
 * faz fallback automático para a ponte local loopback do próprio Tauri.
 */
async function sendToVault(payload) {
  try {
    const nativePromise = browser.runtime.sendNativeMessage(
      NATIVE_HOST_NAME,
      payload
    );
    const timeoutPromise = new Promise((_, reject) =>
      setTimeout(() => reject(new Error("NativeMessaging timeout")), 1200)
    );
    const res = await Promise.race([nativePromise, timeoutPromise]);
    if (res && res.status) {
      return res;
    }
  } catch (err) {
    console.warn(
      "[Password Vault] Native Messaging indisponível no sandbox atual, usando fallback loopback:",
      err?.message || err
    );
  }

  try {
    const resp = await fetch(BRIDGE_FALLBACK_URL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    return await resp.json();
  } catch {
    return {
      status: "locked",
      message: "O aplicativo Password Vault Desktop está fechado.",
    };
  }
}

browser.runtime.onMessage.addListener((message, _sender) => {
  if (!message || !message.type) return false;

  switch (message.type) {
    case "VAULT_GET_STATUS":
      return sendToVault({ action: "get_status" });

    case "VAULT_LAUNCH_APP":
      return sendToVault({ action: "launch_desktop_app" });

    case "VAULT_LAUNCH_APP":
      return sendToVaultNativeHost({ action: "launch_desktop_app" });

    case "VAULT_GET_LOGINS":
      return sendToVault({
        action: "get_logins_for_domain",
        domain: message.domain,
      });

    case "VAULT_CHECK_LOGIN_EXISTS":
      return sendToVault({
        action: "check_login_exists",
        domain: message.domain,
        username: message.username,
      });

    case "VAULT_SAVE_LOGIN":
      return sendToVault({
        action: "save_login",
        domain: message.domain,
        title: message.title || message.domain,
        username: message.username,
        password: message.password,
      });

    default:
      return Promise.resolve({
        status: "error",
        message: "Tipo de mensagem desconhecido",
      });
  }
});
