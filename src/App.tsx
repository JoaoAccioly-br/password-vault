import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Shield,
  Lock,
  Unlock,
  Plus,
  Search,
  KeyRound,
  Copy,
  Eye,
  EyeOff,
  Trash2,
  RefreshCw,
  Globe,
  Check,
  Terminal,
} from "lucide-react";

interface VaultStatus {
  initialized: boolean;
  unlocked: boolean;
  socket_path: string;
  db_path: string;
}

interface CredentialSummary {
  id: string;
  domain: string;
  title: string;
  username: string;
  created_at: number;
  updated_at: number;
}

interface CredentialDecrypted {
  id: string;
  domain: string;
  title: string;
  username: string;
  password: string;
  notes: string | null;
}

export default function App() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [masterPassword, setMasterPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [authError, setAuthError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const [credentials, setCredentials] = useState<CredentialSummary[]>([]);
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<CredentialDecrypted | null>(null);
  const [showPassword, setShowPassword] = useState(false);
  const [copiedField, setCopiedField] = useState<string | null>(null);

  const [isCreating, setIsCreating] = useState(false);
  const [formTitle, setFormTitle] = useState("");
  const [formDomain, setFormDomain] = useState("");
  const [formUsername, setFormUsername] = useState("");
  const [formPassword, setFormPassword] = useState("");
  const [formNotes, setFormNotes] = useState("");

  const fetchStatus = async () => {
    try {
      const s = await invoke<VaultStatus>("get_vault_status");
      setStatus(s);
      if (s.unlocked) {
        await loadCredentials();
      }
    } catch (err) {
      console.error("Erro ao consultar status do cofre:", err);
    }
  };

  const loadCredentials = async () => {
    try {
      const list = await invoke<CredentialSummary[]>("list_credentials");
      setCredentials(list);
    } catch (err) {
      console.error("Erro ao listar credenciais:", err);
    }
  };

  useEffect(() => {
    fetchStatus();

    const unlistenPromise = listen("vault-updated", () => {
      loadCredentials();
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  const handleSetupOrUnlock = async (e: React.FormEvent) => {
    e.preventDefault();
    setAuthError(null);
    if (!status) return;

    if (!status.initialized && masterPassword !== confirmPassword) {
      setAuthError("As senhas mestras não coincidem.");
      return;
    }

    if (masterPassword.length < 8) {
      setAuthError("A Senha Mestra deve ter pelo menos 8 caracteres.");
      return;
    }

    setLoading(true);
    try {
      if (!status.initialized) {
        await invoke("initialize_vault", { masterPassword });
      } else {
        await invoke("unlock_vault", { masterPassword });
      }
      setMasterPassword("");
      setConfirmPassword("");
      await fetchStatus();
    } catch (err) {
      setAuthError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleLock = async () => {
    await invoke("lock_vault");
    setSelected(null);
    setCredentials([]);
    setShowPassword(false);
    await fetchStatus();
  };

  const handleSelectCredential = async (id: string) => {
    setIsCreating(false);
    setShowPassword(false);
    try {
      const dec = await invoke<CredentialDecrypted>("get_credential_decrypted", {
        id,
      });
      setSelected(dec);
    } catch (err) {
      console.error("Erro ao descriptografar credencial:", err);
    }
  };

  const generateStrongPassword = () => {
    const chars =
      "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+";
    const randomValues = new Uint32Array(24);
    crypto.getRandomValues(randomValues);
    let result = "";
    for (let i = 0; i < randomValues.length; i++) {
      result += chars[randomValues[i] % chars.length];
    }
    setFormPassword(result);
  };

  const handleSaveNew = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formDomain || !formUsername || !formPassword) return;
    try {
      const saved = await invoke<CredentialSummary>("save_credential", {
        domain: formDomain,
        title: formTitle || formDomain,
        username: formUsername,
        password: formPassword,
        notes: formNotes || null,
      });
      setIsCreating(false);
      setFormTitle("");
      setFormDomain("");
      setFormUsername("");
      setFormPassword("");
      setFormNotes("");
      await loadCredentials();
      await handleSelectCredential(saved.id);
    } catch (err) {
      alert("Erro ao salvar: " + String(err));
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("Remover esta credencial permanentemente do cofre?")) return;
    try {
      await invoke("delete_credential", { id });
      setSelected(null);
      await loadCredentials();
    } catch (err) {
      console.error("Erro ao excluir:", err);
    }
  };

  const copyWithAutoClear = async (value: string, label: string) => {
    await navigator.clipboard.writeText(value);
    setCopiedField(label);
    setTimeout(() => setCopiedField(null), 2000);
    // Limpa a área de transferência após 15 segundos se ainda contiver o segredo
    setTimeout(async () => {
      try {
        const current = await navigator.clipboard.readText();
        if (current === value) {
          await navigator.clipboard.writeText("");
        }
      } catch {
        // Ignora se a janela não estiver focada
      }
    }, 15000);
  };

  const filteredCredentials = credentials.filter(
    (c) =>
      c.title.toLowerCase().includes(search.toLowerCase()) ||
      c.domain.toLowerCase().includes(search.toLowerCase()) ||
      c.username.toLowerCase().includes(search.toLowerCase())
  );

  if (!status) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-slate-950 text-slate-400">
        Carregando cofre criptográfico...
      </div>
    );
  }

  if (!status.unlocked) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-slate-950 p-6">
        <div className="w-full max-w-md bg-slate-900 border border-slate-800 rounded-2xl p-8 shadow-2xl">
          <div className="flex flex-col items-center text-center mb-6">
            <div className="w-16 h-16 rounded-2xl bg-indigo-500/10 border border-indigo-500/30 flex items-center justify-center text-indigo-400 mb-4">
              <Shield className="w-8 h-8" />
            </div>
            <h1 className="text-2xl font-bold text-white">
              {status.initialized
                ? "Desbloquear Password Vault"
                : "Criar Cofre Local"}
            </h1>
            <p className="text-sm text-slate-400 mt-1">
              {status.initialized
                ? "Criptografia Argon2id + XChaCha20-Poly1305"
                : "Configure sua Senha Mestra. Ela nunca é salva no disco."}
            </p>
          </div>

          <form onSubmit={handleSetupOrUnlock} className="space-y-4">
            <div>
              <label className="block text-xs font-medium text-slate-400 uppercase tracking-wider mb-1.5">
                Senha Mestra
              </label>
              <input
                type="password"
                value={masterPassword}
                onChange={(e) => setMasterPassword(e.target.value)}
                placeholder="Digite sua Senha Mestra..."
                autoFocus
                className="w-full px-4 py-3 rounded-xl bg-slate-950 border border-slate-800 text-white focus:outline-none focus:border-indigo-500"
              />
            </div>

            {!status.initialized && (
              <div>
                <label className="block text-xs font-medium text-slate-400 uppercase tracking-wider mb-1.5">
                  Confirmar Senha Mestra
                </label>
                <input
                  type="password"
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  placeholder="Repita a Senha Mestra..."
                  className="w-full px-4 py-3 rounded-xl bg-slate-950 border border-slate-800 text-white focus:outline-none focus:border-indigo-500"
                />
              </div>
            )}

            {authError && (
              <div className="p-3 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-sm">
                {authError}
              </div>
            )}

            <button
              type="submit"
              disabled={loading}
              className="w-full py-3 px-4 rounded-xl bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 font-semibold text-white flex items-center justify-center gap-2 transition"
            >
              <Unlock className="w-4 h-4" />
              {loading
                ? "Derivando chave Argon2id..."
                : status.initialized
                ? "Desbloquear Cofre"
                : "Inicializar Cofre"}
            </button>
          </form>

          <div className="mt-6 pt-4 border-t border-slate-800/80 text-xs text-slate-500 space-y-1">
            <div className="flex items-center gap-1.5 truncate">
              <Terminal className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
              <span className="truncate">IPC Socket: {status.socket_path}</span>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="h-screen flex bg-slate-950 text-slate-100 overflow-hidden">
      {/* Sidebar */}
      <aside className="w-80 border-r border-slate-800 bg-slate-900/60 flex flex-col">
        <div className="p-4 border-b border-slate-800 flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="w-9 h-9 rounded-xl bg-indigo-500/15 border border-indigo-500/30 flex items-center justify-center text-indigo-400">
              <Shield className="w-5 h-5" />
            </div>
            <div>
              <h2 className="font-bold text-sm leading-tight">Password Vault</h2>
              <span className="text-[11px] text-emerald-400 flex items-center gap-1">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                Desbloqueado • IPC Ativo
              </span>
            </div>
          </div>
          <button
            onClick={handleLock}
            title="Bloquear Cofre (Limpar RAM)"
            className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition"
          >
            <Lock className="w-4 h-4" />
          </button>
        </div>

        <div className="p-3 border-b border-slate-800 space-y-2">
          <div className="relative">
            <Search className="w-4 h-4 text-slate-500 absolute left-3 top-2.5" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Buscar domínio ou usuário..."
              className="w-full pl-9 pr-3 py-2 text-sm rounded-xl bg-slate-950 border border-slate-800 text-white focus:outline-none focus:border-indigo-500"
            />
          </div>
          <button
            onClick={() => {
              setSelected(null);
              setIsCreating(true);
              generateStrongPassword();
            }}
            className="w-full py-2 px-3 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-sm font-medium text-white flex items-center justify-center gap-2 transition"
          >
            <Plus className="w-4 h-4" />
            Novo Login
          </button>
        </div>

        <div className="flex-1 overflow-y-auto divide-y divide-slate-800/60">
          {filteredCredentials.length === 0 ? (
            <div className="p-8 text-center text-sm text-slate-500">
              Nenhuma credencial encontrada.
            </div>
          ) : (
            filteredCredentials.map((item) => (
              <button
                key={item.id}
                onClick={() => handleSelectCredential(item.id)}
                className={`w-full text-left p-3.5 hover:bg-slate-800/50 transition flex items-center gap-3 ${
                  selected?.id === item.id ? "bg-indigo-500/10 border-l-2 border-indigo-500" : ""
                }`}
              >
                <div className="w-9 h-9 rounded-lg bg-slate-800 flex items-center justify-center text-slate-300 shrink-0">
                  <Globe className="w-4 h-4" />
                </div>
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium text-white truncate">
                    {item.title}
                  </p>
                  <p className="text-xs text-slate-400 truncate">
                    {item.username} • {item.domain}
                  </p>
                </div>
              </button>
            ))
          )}
        </div>
      </aside>

      {/* Main Content */}
      <main className="flex-1 overflow-y-auto p-8">
        {isCreating ? (
          <div className="max-w-xl mx-auto bg-slate-900 border border-slate-800 rounded-2xl p-6">
            <h2 className="text-lg font-bold mb-4 flex items-center gap-2">
              <KeyRound className="w-5 h-5 text-indigo-400" />
              Adicionar Credencial
            </h2>
            <form onSubmit={handleSaveNew} className="space-y-4">
              <div>
                <label className="block text-xs text-slate-400 uppercase mb-1">
                  Título (Ex: GitHub Pessoal)
                </label>
                <input
                  type="text"
                  value={formTitle}
                  onChange={(e) => setFormTitle(e.target.value)}
                  placeholder="GitHub"
                  className="w-full px-3.5 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm"
                />
              </div>
              <div>
                <label className="block text-xs text-slate-400 uppercase mb-1">
                  Domínio / URL *
                </label>
                <input
                  type="text"
                  required
                  value={formDomain}
                  onChange={(e) => setFormDomain(e.target.value)}
                  placeholder="github.com"
                  className="w-full px-3.5 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm"
                />
              </div>
              <div>
                <label className="block text-xs text-slate-400 uppercase mb-1">
                  Usuário / E-mail *
                </label>
                <input
                  type="text"
                  required
                  value={formUsername}
                  onChange={(e) => setFormUsername(e.target.value)}
                  placeholder="seu@email.com"
                  className="w-full px-3.5 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm"
                />
              </div>
              <div>
                <label className="block text-xs text-slate-400 uppercase mb-1">
                  Senha *
                </label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    required
                    value={formPassword}
                    onChange={(e) => setFormPassword(e.target.value)}
                    className="flex-1 px-3.5 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm font-mono"
                  />
                  <button
                    type="button"
                    onClick={generateStrongPassword}
                    title="Gerar senha aleatória CSPRNG"
                    className="px-3 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-200"
                  >
                    <RefreshCw className="w-4 h-4" />
                  </button>
                </div>
              </div>
              <div>
                <label className="block text-xs text-slate-400 uppercase mb-1">
                  Notas Criptografadas (Opcional)
                </label>
                <textarea
                  rows={3}
                  value={formNotes}
                  onChange={(e) => setFormNotes(e.target.value)}
                  className="w-full px-3.5 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm"
                />
              </div>
              <div className="flex justify-end gap-2 pt-2">
                <button
                  type="button"
                  onClick={() => setIsCreating(false)}
                  className="px-4 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-sm"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  className="px-5 py-2 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-sm font-semibold text-white"
                >
                  Criptografar e Salvar
                </button>
              </div>
            </form>
          </div>
        ) : selected ? (
          <div className="max-w-xl mx-auto bg-slate-900 border border-slate-800 rounded-2xl p-6 space-y-6">
            <div className="flex items-start justify-between border-b border-slate-800 pb-4">
              <div>
                <h2 className="text-xl font-bold text-white">{selected.title}</h2>
                <p className="text-sm text-indigo-400">{selected.domain}</p>
              </div>
              <button
                onClick={() => handleDelete(selected.id)}
                className="p-2 rounded-lg text-rose-400 hover:bg-rose-500/10 transition"
                title="Excluir credencial"
              >
                <Trash2 className="w-4 h-4" />
              </button>
            </div>

            <div className="space-y-4">
              <div className="p-3.5 rounded-xl bg-slate-950 border border-slate-800 flex items-center justify-between">
                <div>
                  <span className="block text-[11px] uppercase text-slate-500">
                    Usuário / Login
                  </span>
                  <span className="text-sm font-medium text-white">
                    {selected.username}
                  </span>
                </div>
                <button
                  onClick={() => copyWithAutoClear(selected.username, "user")}
                  className="p-2 rounded-lg hover:bg-slate-800 text-slate-400 hover:text-white"
                >
                  {copiedField === "user" ? (
                    <Check className="w-4 h-4 text-emerald-400" />
                  ) : (
                    <Copy className="w-4 h-4" />
                  )}
                </button>
              </div>

              <div className="p-3.5 rounded-xl bg-slate-950 border border-slate-800 flex items-center justify-between">
                <div className="min-w-0 flex-1 pr-3">
                  <span className="block text-[11px] uppercase text-slate-500">
                    Senha (XChaCha20-Poly1305)
                  </span>
                  <span className="text-sm font-mono text-white break-all">
                    {showPassword ? selected.password : "••••••••••••••••••••"}
                  </span>
                </div>
                <div className="flex items-center gap-1">
                  <button
                    onClick={() => setShowPassword(!showPassword)}
                    className="p-2 rounded-lg hover:bg-slate-800 text-slate-400 hover:text-white"
                  >
                    {showPassword ? (
                      <EyeOff className="w-4 h-4" />
                    ) : (
                      <Eye className="w-4 h-4" />
                    )}
                  </button>
                  <button
                    onClick={() => copyWithAutoClear(selected.password, "pass")}
                    className="p-2 rounded-lg hover:bg-slate-800 text-slate-400 hover:text-white"
                  >
                    {copiedField === "pass" ? (
                      <Check className="w-4 h-4 text-emerald-400" />
                    ) : (
                      <Copy className="w-4 h-4" />
                    )}
                  </button>
                </div>
              </div>

              {selected.notes && (
                <div className="p-3.5 rounded-xl bg-slate-950 border border-slate-800">
                  <span className="block text-[11px] uppercase text-slate-500 mb-1">
                    Notas
                  </span>
                  <p className="text-sm text-slate-300 whitespace-pre-wrap">
                    {selected.notes}
                  </p>
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="h-full flex flex-col items-center justify-center text-center text-slate-500">
            <Shield className="w-12 h-12 text-slate-700 mb-3" />
            <p className="text-base font-medium text-slate-400">
              Selecione uma credencial ou adicione um novo login
            </p>
            <p className="text-xs text-slate-600 max-w-md mt-1">
              A extensão do Firefox / Zen Browser se comunica automaticamente
              com este cofre via Native Messaging + Unix Socket.
            </p>
          </div>
        )}
      </main>
    </div>
  );
}
