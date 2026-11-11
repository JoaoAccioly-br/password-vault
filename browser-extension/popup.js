async function initPopup() {
  const badge = document.getElementById("status-badge");
  const domainLabel = document.getElementById("domain-label");
  const listEl = document.getElementById("logins-list");
  const openAppBtn = document.getElementById("open-app-btn");

  openAppBtn.onclick = async () => {
    openAppBtn.textContent = "Abrindo janela...";
    await browser.runtime.sendMessage({ type: "VAULT_LAUNCH_APP" });
    setTimeout(() => window.close(), 400);
  };

  const tabs = await browser.tabs.query({ active: true, currentWindow: true });
  const activeTab = tabs[0];
  let currentDomain = "";

  try {
    if (activeTab && activeTab.url) {
      const urlObj = new URL(activeTab.url);
      currentDomain = urlObj.hostname.replace(/^www\./, "");
    }
  } catch {
    currentDomain = "";
  }

  domainLabel.textContent = currentDomain
    ? `Site atual: ${currentDomain}`
    : "Aba sem domínio HTTP/HTTPS";

  const statusRes = await browser.runtime.sendMessage({
    type: "VAULT_GET_STATUS",
  });

  if (!statusRes || statusRes.status !== "ok" || !statusRes.unlocked) {
    badge.textContent = "Bloqueado";
    badge.className = "badge badge-locked";
    listEl.innerHTML = `<div class="empty">${
      statusRes?.message ||
      "O cofre está fechado ou bloqueado. Clique abaixo para abrir a janela Desktop."
    }</div>`;
    return;
  }

  badge.textContent = "Desbloqueado";
  badge.className = "badge badge-ok";

  if (!currentDomain) {
    listEl.innerHTML = `<div class="empty">Abra um site para preencher credenciais automaticamente.</div>`;
    return;
  }

  const loginsRes = await browser.runtime.sendMessage({
    type: "VAULT_GET_LOGINS",
    domain: currentDomain,
  });

  if (
    !loginsRes ||
    loginsRes.status !== "ok" ||
    !Array.isArray(loginsRes.logins) ||
    loginsRes.logins.length === 0
  ) {
    listEl.innerHTML = `<div class="empty">Nenhuma senha salva para <strong>${currentDomain}</strong>.</div>`;
    return;
  }

  listEl.innerHTML = "";
  loginsRes.logins.forEach((cred) => {
    const row = document.createElement("div");
    row.className = "login-item";

    const info = document.createElement("div");
    info.className = "login-info";

    const user = document.createElement("div");
    user.className = "login-user";
    user.textContent = cred.username;

    const dom = document.createElement("div");
    dom.className = "login-domain";
    dom.textContent = cred.domain;

    info.appendChild(user);
    info.appendChild(dom);

    const btn = document.createElement("button");
    btn.className = "btn-fill";
    btn.textContent = "Preencher";
    btn.onclick = async () => {
      if (activeTab && activeTab.id) {
        await browser.tabs.sendMessage(activeTab.id, {
          type: "VAULT_FILL_CREDENTIAL",
          credential: cred,
        });
        window.close();
      }
    };

    row.appendChild(info);
    row.appendChild(btn);
    listEl.appendChild(row);
  });
}

document.addEventListener("DOMContentLoaded", initPopup);
