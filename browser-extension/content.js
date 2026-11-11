(function () {
  const currentDomain = window.location.hostname.replace(/^www\./, "");
  let cachedLogins = [];

  /**
   * Preenche um input disparando eventos compatíveis com React / Vue / Angular.
   */
  function fillInput(input, value) {
    if (!input) return;
    input.focus();
    const nativeSetter = Object.getOwnPropertyDescriptor(
      window.HTMLInputElement.prototype,
      "value"
    )?.set;
    if (nativeSetter) {
      nativeSetter.call(input, value);
    } else {
      input.value = value;
    }
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
  }

  /**
   * Localiza o campo de usuário/e-mail mais próximo do campo de senha.
   */
  function findUsernameInput(passwordInput) {
    const scope = passwordInput.form || document;
    const candidates = Array.from(
      scope.querySelectorAll(
        'input[type="text"], input[type="email"], input:not([type])'
      )
    ).filter(
      (el) =>
        el !== passwordInput &&
        el.offsetParent !== null &&
        !el.disabled &&
        !el.readOnly
    );

    if (candidates.length === 0) return null;
    return candidates[candidates.length - 1];
  }

  /**
   * Preenche o formulário na página com a credencial escolhida.
   */
  function applyCredentialToPage(cred, targetPasswordInput) {
    const pwInputs = targetPasswordInput
      ? [targetPasswordInput]
      : Array.from(document.querySelectorAll('input[type="password"]'));

    for (const pwInput of pwInputs) {
      if (pwInput.offsetParent === null) continue;
      const userField = findUsernameInput(pwInput);
      if (userField) {
        fillInput(userField, cred.username);
      }
      fillInput(pwInput, cred.password);
    }
  }

  /**
   * Injeta um botão sutil de Auto-fill ao focar em um campo de senha quando há logins disponíveis.
   */
  function attachAutofillHelper(pwInput) {
    if (pwInput.dataset.vaultAttached === "true") return;
    pwInput.dataset.vaultAttached = "true";

    pwInput.addEventListener("focus", async () => {
      const res = await browser.runtime.sendMessage({
        type: "VAULT_GET_LOGINS",
        domain: currentDomain,
      });

      if (res && res.status === "ok" && Array.isArray(res.logins) && res.logins.length > 0) {
        cachedLogins = res.logins;
        showAutofillPopover(pwInput, cachedLogins);
      }
    });
  }

  function showAutofillPopover(pwInput, logins) {
    const existing = document.getElementById("pv-autofill-popover");
    if (existing) existing.remove();

    const rect = pwInput.getBoundingClientRect();
    const popover = document.createElement("div");
    popover.id = "pv-autofill-popover";
    Object.assign(popover.style, {
      position: "fixed",
      top: `${rect.bottom + 6}px`,
      left: `${rect.left}px`,
      minWidth: `${Math.max(rect.width, 240)}px`,
      zIndex: "2147483647",
      background: "#0f172a",
      color: "#f8fafc",
      border: "1px solid #334155",
      borderRadius: "10px",
      boxShadow: "0 10px 25px rgba(0,0,0,0.5)",
      padding: "6px",
      fontFamily: "system-ui, -apple-system, sans-serif",
      fontSize: "13px",
    });

    const header = document.createElement("div");
    header.textContent = "🔐 Password Vault — Preencher:";
    Object.assign(header.style, {
      fontSize: "11px",
      color: "#94a3b8",
      padding: "4px 8px",
      borderBottom: "1px solid #1e293b",
      marginBottom: "4px",
    });
    popover.appendChild(header);

    logins.forEach((cred) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.textContent = `${cred.username} (${cred.domain})`;
      Object.assign(btn.style, {
        display: "block",
        width: "100%",
        textAlign: "left",
        padding: "7px 8px",
        background: "transparent",
        color: "#f1f5f9",
        border: "none",
        borderRadius: "6px",
        cursor: "pointer",
      });
      btn.addEventListener("mouseenter", () => {
        btn.style.background = "#1e293b";
      });
      btn.addEventListener("mouseleave", () => {
        btn.style.background = "transparent";
      });
      btn.addEventListener("mousedown", (e) => {
        e.preventDefault();
        applyCredentialToPage(cred, pwInput);
        popover.remove();
      });
      popover.appendChild(btn);
    });

    document.body.appendChild(popover);

    const closeHandler = (ev) => {
      if (!popover.contains(ev.target) && ev.target !== pwInput) {
        popover.remove();
        document.removeEventListener("mousedown", closeHandler);
      }
    };
    setTimeout(() => document.addEventListener("mousedown", closeHandler), 50);
  }

  /**
   * Exibe um banner flutuante no canto superior direito perguntando se o usuário quer salvar a senha.
   */
  function showSaveBanner(domain, username, password) {
    const existing = document.getElementById("pv-save-banner");
    if (existing) existing.remove();

    const banner = document.createElement("div");
    banner.id = "pv-save-banner";
    Object.assign(banner.style, {
      position: "fixed",
      top: "16px",
      right: "16px",
      width: "340px",
      zIndex: "2147483647",
      background: "#0f172a",
      color: "#f8fafc",
      border: "1px solid #4f46e5",
      borderRadius: "12px",
      boxShadow: "0 15px 35px rgba(0,0,0,0.65)",
      padding: "16px",
      fontFamily: "system-ui, -apple-system, sans-serif",
      fontSize: "13px",
    });

    const title = document.createElement("div");
    title.textContent = "🛡️ Salvar no Password Vault?";
    Object.assign(title.style, {
      fontWeight: "700",
      fontSize: "14px",
      marginBottom: "6px",
      color: "#ffffff",
    });

    const desc = document.createElement("div");
    desc.textContent = `${username} em ${domain}`;
    Object.assign(desc.style, {
      color: "#94a3b8",
      fontSize: "12px",
      marginBottom: "12px",
      wordBreak: "break-all",
    });

    const actions = document.createElement("div");
    Object.assign(actions.style, {
      display: "flex",
      justifyContent: "flex-end",
      gap: "8px",
    });

    const dismissBtn = document.createElement("button");
    dismissBtn.textContent = "Agora não";
    Object.assign(dismissBtn.style, {
      padding: "6px 12px",
      borderRadius: "8px",
      background: "#1e293b",
      color: "#cbd5e1",
      border: "none",
      cursor: "pointer",
    });
    dismissBtn.onclick = () => banner.remove();

    const saveBtn = document.createElement("button");
    saveBtn.textContent = "Salvar Senha";
    Object.assign(saveBtn.style, {
      padding: "6px 14px",
      borderRadius: "8px",
      background: "#4f46e5",
      color: "#ffffff",
      fontWeight: "600",
      border: "none",
      cursor: "pointer",
    });
    saveBtn.onclick = async () => {
      saveBtn.textContent = "Salvando...";
      const res = await browser.runtime.sendMessage({
        type: "VAULT_SAVE_LOGIN",
        domain,
        title: document.title || domain,
        username,
        password,
      });
      if (res && res.status === "ok") {
        title.textContent = "✅ Senha criptografada e salva!";
        actions.remove();
        setTimeout(() => banner.remove(), 1800);
      } else {
        desc.textContent =
          res?.message || "Desbloqueie o aplicativo Desktop para salvar.";
        saveBtn.textContent = "Tentar novamente";
      }
    };

    actions.appendChild(dismissBtn);
    actions.appendChild(saveBtn);
    banner.appendChild(title);
    banner.appendChild(desc);
    banner.appendChild(actions);
    document.body.appendChild(banner);
  }

  /**
   * Intercepta submissões de formulário para sugerir Auto-save.
   */
  document.addEventListener(
    "submit",
    async (event) => {
      const form = event.target;
      if (!(form instanceof HTMLFormElement)) return;

      const pwInput = form.querySelector('input[type="password"]');
      if (!pwInput || !pwInput.value) return;

      const userField = findUsernameInput(pwInput);
      const username = userField?.value?.trim();
      const password = pwInput.value;

      if (!username || !password) return;

      const check = await browser.runtime.sendMessage({
        type: "VAULT_CHECK_LOGIN_EXISTS",
        domain: currentDomain,
        username,
      });

      // Se o cofre estiver desbloqueado e o login ainda não existir, exibe o banner de salvamento
      if (!check || check.exists !== true) {
        showSaveBanner(currentDomain, username, password);
      }
    },
    true
  );

  /**
   * Escuta comandos vindos do Popup da extensão (ex: preenchimento com 1 clique).
   */
  browser.runtime.onMessage.addListener((msg) => {
    if (msg && msg.type === "VAULT_FILL_CREDENTIAL" && msg.credential) {
      applyCredentialToPage(msg.credential, null);
    }
  });

  // Inicializa observação de campos de senha na página (suporta SPAs)
  function scanPasswordFields() {
    document
      .querySelectorAll('input[type="password"]')
      .forEach(attachAutofillHelper);
  }

  scanPasswordFields();
  const observer = new MutationObserver(() => scanPasswordFields());
  observer.observe(document.documentElement, {
    childList: true,
    subtree: true,
  });
})();
