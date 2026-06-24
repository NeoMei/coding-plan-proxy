import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ThemeProvider, useTheme } from "./contexts/ThemeContext";
import { t, setLocale, getLocale } from "./data/locale";
import { BUILTIN_PRESETS, findPreset } from "./data/presets";

interface Provider {
  id: string;
  name: string;
  model: string;
  upstream: string;
  api_key: string;
  context_window: number;
  max_output_tokens: number;
  enabled: boolean;
  sort_index: number;
  verified: boolean;
}

function AppShell() {
  return (
    <ThemeProvider>
      <App />
    </ThemeProvider>
  );
}

function App() {
  const { theme, toggle: toggleTheme } = useTheme();
  const [locale, setLoc] = useState(getLocale());
  const [providers, setProviders] = useState<Provider[]>([]);
  const [proxyRunning, setProxyRunning] = useState(false);
  const [proxyPort, setProxyPort] = useState(15731);
  const [codexTest, setCodexTest] = useState<string | null>(null);
  const [editing, setEditing] = useState<Provider | null>(null);
  const [showAdd, setShowAdd] = useState(false);
  const [quickSetup, setQuickSetup] = useState<string | null>(null); // preset model slug
  const [testResult, setTestResult] = useState<Record<string, string>>({});
  const [testing, setTesting] = useState<Record<string, boolean>>({});
  const [autoStart, setAutoStart] = useState(false);
  const [statusMsg, setStatusMsg] = useState("");

  const refreshProviders = useCallback(async () => {
    const list = await invoke<Provider[]>("list_providers");
    setProviders(list);
  }, []);

  const refreshStatus = useCallback(async () => {
    const running = await invoke<boolean>("proxy_status");
    setProxyRunning(running);
    try { setProxyPort(await invoke<number>("proxy_port")); } catch {}
  }, []);

  useEffect(() => { refreshProviders(); refreshStatus(); }, [refreshProviders, refreshStatus]);
  useEffect(() => {
    const timer = setInterval(refreshStatus, 5000);
    return () => clearInterval(timer);
  }, [refreshStatus]);

  useEffect(() => {
    invoke<string>("get_setting", { key: "auto_start" }).then(v => setAutoStart(v === "true")).catch(() => {});
  }, []);

  const switchLocale = (l: string) => { setLoc(l); setLocale(l); };

  const toggleProxy = async () => {
    try {
      if (proxyRunning) {
        setStatusMsg("Stopping...");
        await invoke("stop_proxy");
        // Wait and verify
        await new Promise(r => setTimeout(r, 1000));
        setStatusMsg("");
      } else {
        if (!hasKeys) {
          setStatusMsg("Add a provider with an API key first");
          return;
        }
        setStatusMsg("Starting...");
        await invoke("start_proxy");
        setStatusMsg("");
      }
      refreshStatus();
    } catch (e: any) {
      setStatusMsg(String(e));
    }
  };

  const testProvider = async (p: Provider) => {
    setTesting(t => ({ ...t, [p.id]: true }));
    setStatusMsg(`Testing ${p.name}...`);
    try {
      await invoke<string>("test_connection", { provider: p });
      setTestResult(t => ({ ...t, [p.id]: "ok" }));
      await invoke("set_verified", { id: p.id, verified: true });
      setStatusMsg(`✓ ${p.name}: connected`);
    } catch (e: any) {
      setTestResult(t => ({ ...t, [p.id]: "fail" }));
      await invoke("set_verified", { id: p.id, verified: false });
      setStatusMsg(`✗ ${p.name}: ${String(e).slice(0, 80)}`);
    }
    setTesting(t => ({ ...t, [p.id]: false }));
    await refreshProviders();
    setTimeout(() => setStatusMsg(""), 3000);
  };

  // Sort: connected providers first, then untested, then failed
  const sortedProviders = [...providers].sort((a, b) => {
    const sa = testResult[a.id];
    const sb = testResult[b.id];
    if (sa === "ok" && sb !== "ok") return -1;
    if (sa !== "ok" && sb === "ok") return 1;
    if (sa === "fail" && sb !== "fail") return 1;
    if (sa !== "fail" && sb === "fail") return -1;
    return a.sort_index - b.sort_index;
  });

  const hasKeys = providers.some(p => p.api_key.length > 4);

  const applyModel = async (model: string) => {
    await invoke("apply_to_codex", { model });
    refreshStatus();
  };

  const saveProvider = async (p: Provider) => {
    if (!p.id) p.id = await invoke<string>("generate_id");
    await invoke("save_provider", { provider: p });
    setEditing(null); setShowAdd(false); setQuickSetup(null);
    refreshProviders();
  };

  const deleteProvider = async (id: string) => {
    await invoke("delete_provider", { id });
    refreshProviders();
  };

  const toggleAutoStart = async () => {
    const next = !autoStart;
    setAutoStart(next);
    await invoke("set_setting", { key: "auto_start", value: String(next) });
  };

  const openQuickSetup = (model: string) => {
    const preset = findPreset(model);
    if (preset) {
      setEditing({
        id: "", name: preset.name, model: preset.model, upstream: preset.upstream,
        api_key: "", context_window: preset.contextWindow, max_output_tokens: preset.maxOutputTokens,
        enabled: true, sort_index: 0, verified: false,
      });
    }
    setQuickSetup(model);
    setShowAdd(false);
  };

  const emptyProvider = (): Provider => ({
    id: "", name: "", model: "", upstream: "", api_key: "",
    context_window: 262144, max_output_tokens: 32768, enabled: true, sort_index: 0, verified: false,
  });

  return (
    <div className={`h-screen flex flex-col ${theme === "light" ? "bg-white text-zinc-900" : "bg-zinc-950 text-zinc-200"}`}>
      {/* Header */}
      <header className={`flex items-center justify-between px-5 py-2.5 border-b ${theme === "light" ? "border-zinc-200 bg-zinc-50" : "border-zinc-800 bg-zinc-950"}`}>
        <div className="flex items-center gap-3">
          <h1 className="text-base font-semibold tracking-tight">{t("app.title")}</h1>
          <span className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium ${
            proxyRunning
              ? (theme === "light" ? "bg-emerald-100 text-emerald-700" : "bg-emerald-900/40 text-emerald-400")
              : (theme === "light" ? "bg-zinc-100 text-zinc-500" : "bg-zinc-800 text-zinc-500")
          }`}>
            <span className={`w-2 h-2 rounded-full ${proxyRunning ? "bg-emerald-500 animate-pulse" : "bg-zinc-400"}`} />
            {proxyRunning ? `${t("proxy.running")} :${proxyPort}` : t("proxy.stopped")}
          </span>
          {statusMsg && (
            <span className="ml-2 text-xs text-red-500 max-w-xs truncate">{statusMsg}</span>
          )}
        </div>
        <div className="flex items-center gap-1.5">
          {/* Locale toggle */}
          <button onClick={() => switchLocale(locale === "zh" ? "en" : "zh")}
            className={`px-2 py-1 text-xs rounded border transition ${theme === "light" ? "border-zinc-200 hover:bg-zinc-100" : "border-zinc-700 hover:bg-zinc-800"}`}>
            {locale === "zh" ? "EN" : "中"}
          </button>
          {/* Theme toggle */}
          <button onClick={toggleTheme}
            className={`px-2 py-1 text-xs rounded border transition ${theme === "light" ? "border-zinc-200 hover:bg-zinc-100" : "border-zinc-700 hover:bg-zinc-800"}`}>
            {theme === "dark" ? "☀" : "☾"}
          </button>
          {/* Proxy toggle */}
          <button onClick={toggleProxy} disabled={!proxyRunning && !hasKeys}
            title={!proxyRunning && !hasKeys ? "Add a provider with an API key first" : ""}
            className={`px-4 py-1.5 rounded text-sm font-medium transition ${
              proxyRunning
                ? "bg-red-600/10 text-red-600 border border-red-600/20 hover:bg-red-600/20"
                : hasKeys
                  ? "bg-emerald-600/10 text-emerald-600 border border-emerald-600/20 hover:bg-emerald-600/20"
                  : "bg-zinc-700/20 text-zinc-500 border border-zinc-700/30 cursor-not-allowed"
            }`}>
            {proxyRunning ? t("proxy.stop") : t("proxy.start")}
          </button>
        </div>
      </header>

      {/* Main */}
      <main className="flex-1 overflow-auto p-5">
        {/* Providers */}
        <div className="mb-5">
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-zinc-400">{t("providers.title")}</h2>
            <button onClick={() => { setEditing(emptyProvider()); setShowAdd(true); setQuickSetup(null); }}
              className={`px-3 py-1 text-sm rounded border transition ${
                theme === "light"
                  ? "border-blue-200 text-blue-600 hover:bg-blue-50"
                  : "border-blue-600/30 text-blue-400 hover:bg-blue-600/20"
              }`}>
              {t("providers.add")}
            </button>
          </div>

          <div className="space-y-2">
            {sortedProviders.map(p => {
              const hasKey = p.api_key.length > 4;
              const testState = testResult[p.id];
              return (
                <div key={p.id}
                  className={`flex items-center gap-3 p-3 rounded-lg border transition ${
                    theme === "light"
                      ? (p.enabled ? "border-zinc-200 bg-white" : "border-zinc-100 bg-zinc-50 opacity-60")
                      : (p.enabled ? "border-zinc-700/50 bg-zinc-900/50" : "border-zinc-800 bg-zinc-900/20 opacity-60")
                  }`}>
                  {/* Provider icon */}
                  <div className={`w-8 h-8 rounded-lg flex items-center justify-center text-sm font-bold shrink-0 ${
                    theme === "light" ? "bg-zinc-100 text-zinc-600" : "bg-zinc-800 text-zinc-400"
                  }`}>
                    {p.name.charAt(0)}
                  </div>
                  {/* Info */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-sm">{p.name}</span>
                      <code className={`text-xs px-1.5 py-0.5 rounded ${theme === "light" ? "bg-zinc-100 text-zinc-500" : "bg-zinc-800 text-zinc-400"}`}>{p.model}</code>
                      <span className={`text-xs ${hasKey ? "text-emerald-500" : "text-amber-500"}`}>
                        {hasKey ? t("providers.keySet") : t("providers.keyMissing")}
                      </span>
                    </div>
                    <div className="text-xs text-zinc-500 mt-0.5 truncate">{p.upstream}</div>
                    {testState && (
                      <span className={`text-xs ${testState === "ok" ? "text-emerald-500" : "text-red-500"}`}>
                        {testState === "ok" ? t("providers.testOk") : t("providers.testFail")}
                      </span>
                    )}
                  </div>
                  {/* Actions */}
                  <div className="flex items-center gap-1 shrink-0">
                    {p.enabled && proxyRunning && (
                      <button onClick={() => applyModel(p.model)}
                        className={`text-xs px-2 py-1 rounded border transition ${
                          theme === "light"
                            ? "border-violet-200 text-violet-600 hover:bg-violet-50"
                            : "border-violet-600/30 text-violet-400 hover:bg-violet-600/20"
                        }`}>
                        {t("providers.apply")}
                      </button>
                    )}
                    <button onClick={() => testProvider(p)} disabled={testing[p.id]}
                      className={`p-1.5 text-xs rounded transition disabled:opacity-50 ${theme === "light" ? "hover:bg-zinc-100 text-zinc-400" : "hover:bg-zinc-700 text-zinc-400"}`}>
                      {testing[p.id] ? "⏳" : "🔍"}
                    </button>
                    <button onClick={() => { setEditing({ ...p }); setShowAdd(false); setQuickSetup(null); }}
                      className={`p-1.5 text-xs rounded transition ${theme === "light" ? "hover:bg-zinc-100 text-zinc-400" : "hover:bg-zinc-700 text-zinc-400"}`}>
                      ✏️
                    </button>
                    <button onClick={() => deleteProvider(p.id)}
                      className={`p-1.5 text-xs rounded transition ${theme === "light" ? "hover:bg-red-50 text-zinc-400 hover:text-red-500" : "hover:bg-zinc-700 text-zinc-400 hover:text-red-400"}`}>
                      🗑
                    </button>
                  </div>
                </div>
              );
            })}
            {sortedProviders.length === 0 && (
              <div className={`text-center py-12 text-sm ${theme === "light" ? "text-zinc-400" : "text-zinc-600"}`}>
                {t("providers.empty")}
              </div>
            )}
          </div>
        </div>

        {/* Settings */}
        <div className={`border-t pt-5 ${theme === "light" ? "border-zinc-200" : "border-zinc-800"}`}>
          <h2 className="text-xs font-semibold uppercase tracking-wider text-zinc-400 mb-3">{t("settings.title")}</h2>
          <div className="space-y-3 max-w-md">
            {/* Auto-start */}
            <label className={`flex items-center justify-between p-3 rounded-lg border ${theme === "light" ? "border-zinc-200" : "border-zinc-700/50 bg-zinc-900/50"}`}>
              <div>
                <div className="text-sm font-medium">{t("settings.autoStart")}</div>
                <div className="text-xs text-zinc-500">{t("settings.autoStartDesc")}</div>
              </div>
              <button onClick={toggleAutoStart}
                className={`w-10 h-5 rounded-full transition relative ${autoStart ? "bg-blue-600" : (theme === "light" ? "bg-zinc-300" : "bg-zinc-700")}`}>
                <span className={`absolute top-0.5 w-4 h-4 rounded-full bg-white transition ${autoStart ? "left-5" : "left-0.5"}`} />
              </button>
            </label>
            {/* Codex connectivity test */}
            <div className="mt-3">
              <button
                onClick={async () => {
                  try {
                    const resp = await fetch("http://127.0.0.1:15731/health");
                    if (resp.ok) setCodexTest("ok");
                    else setCodexTest("fail");
                  } catch { setCodexTest("fail"); }
                }}
                className={`px-3 py-1.5 text-sm rounded border transition ${
                  theme === "light"
                    ? "border-zinc-200 hover:bg-zinc-50"
                    : "border-zinc-700 hover:bg-zinc-800"
                }`}>
                Test Codex → Proxy Connection
              </button>
              {codexTest === "ok" && <span className="ml-2 text-xs text-emerald-500">✓ Codex can reach proxy</span>}
              {codexTest === "fail" && <span className="ml-2 text-xs text-red-500">✗ Cannot reach proxy — start the proxy first</span>}
            </div>
          </div>
        </div>
      </main>

      {/* Modal: Add with preset selector */}
      {showAdd && !quickSetup && (
        <Modal onClose={() => { setShowAdd(false); setEditing(null); }} title={t("modal.addTitle")} theme={theme}>
          {/* Preset quick-pick */}
          <div className="mb-4">
            <label className="text-xs text-zinc-500 mb-2 block">{t("preset.select")}</label>
            <div className="grid grid-cols-2 gap-1.5 max-h-48 overflow-y-auto">
              {BUILTIN_PRESETS.map(p => (
                <button key={p.id}
                  onClick={() => openQuickSetup(p.model)}
                  className={`text-left px-3 py-2 rounded text-xs border transition truncate ${
                    theme === "light"
                      ? "border-zinc-200 hover:border-blue-300 hover:bg-blue-50"
                      : "border-zinc-700 hover:border-blue-600 hover:bg-blue-600/10"
                  }`}>
                  <div className="font-medium">{p.label}</div>
                  <div className="text-zinc-500">{p.model}</div>
                </button>
              ))}
              <button
                onClick={() => { setQuickSetup("custom"); setEditing(emptyProvider()); }}
                className={`text-left px-3 py-2 rounded text-xs border transition ${
                  theme === "light" ? "border-zinc-200 hover:border-zinc-300" : "border-zinc-700 hover:border-zinc-600"
                }`}>
                <div className="font-medium">{t("preset.custom")}</div>
                <div className="text-zinc-500">—</div>
              </button>
            </div>
          </div>
        </Modal>
      )}

      {/* Modal: Quick key-only or full edit */}
      {(editing && (quickSetup || showAdd)) && (
        <ProviderEditor
          provider={editing}
          isQuick={quickSetup !== null && quickSetup !== "custom"}
          onSave={saveProvider}
          onClose={() => { setEditing(null); setShowAdd(false); setQuickSetup(null); }}
          theme={theme}
        />
      )}

      {/* Modal: Edit existing */}
      {editing && !showAdd && !quickSetup && (
        <ProviderEditor
          provider={editing}
          isQuick={false}
          onSave={saveProvider}
          onClose={() => setEditing(null)}
          theme={theme}
        />
      )}
    </div>
  );
}

function Modal({ children, onClose, title, theme }: {
  children: React.ReactNode; onClose: () => void; title: string; theme: string;
}) {
  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={onClose}>
      <div className={`rounded-xl p-5 w-full max-w-md mx-4 ${theme === "light" ? "bg-white border border-zinc-200" : "bg-zinc-900 border border-zinc-700"}`}
        onClick={e => e.stopPropagation()}>
        <h3 className="text-base font-semibold mb-4">{title}</h3>
        {children}
      </div>
    </div>
  );
}

function ProviderEditor({ provider, isQuick, onSave, onClose, theme }: {
  provider: Provider; isQuick: boolean; onSave: (p: Provider) => void; onClose: () => void; theme: string;
}) {
  const [form, setForm] = useState({ ...provider });

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={onClose}>
      <div className={`rounded-xl p-5 w-full max-w-md mx-4 ${theme === "light" ? "bg-white border border-zinc-200" : "bg-zinc-900 border border-zinc-700"}`}
        onClick={e => e.stopPropagation()}>
        <h3 className="text-base font-semibold mb-1">
          {provider.id ? t("modal.editTitle") : t("modal.addTitle")}
        </h3>
        {isQuick && (
          <p className="text-xs text-emerald-500 mb-3">{t("modal.quickSetup")}: {t("modal.apiKeyOnly")}</p>
        )}

        <div className="space-y-3">
          {!isQuick && (
            <>
              <Field label={t("field.name")} value={form.name} onChange={v => setForm(f => ({ ...f, name: v }))} theme={theme} />
              <Field label={t("field.model")} value={form.model} onChange={v => setForm(f => ({ ...f, model: v }))} theme={theme} />
              <Field label={t("field.upstream")} value={form.upstream} onChange={v => setForm(f => ({ ...f, upstream: v }))} theme={theme} />
            </>
          )}
          {isQuick && (
            <div className={`p-2 rounded text-xs ${theme === "light" ? "bg-zinc-50" : "bg-zinc-800"}`}>
              <span className="font-medium">{form.name}</span>
              <span className="text-zinc-500 mx-2">→</span>
              <code className="text-zinc-500">{form.model}</code>
            </div>
          )}
          <Field label={t("field.apiKey")} value={form.api_key} onChange={v => setForm(f => ({ ...f, api_key: v }))}
            type="password" placeholder="sk-..." theme={theme} autoFocus={isQuick} />
          {!isQuick && (
            <div className="grid grid-cols-2 gap-3">
              <Field label={t("field.contextWindow")} value={String(form.context_window)}
                onChange={v => setForm(f => ({ ...f, context_window: Number(v) || 262144 }))} theme={theme} />
              <Field label={t("field.maxTokens")} value={String(form.max_output_tokens)}
                onChange={v => setForm(f => ({ ...f, max_output_tokens: Number(v) || 32768 }))} theme={theme} />
            </div>
          )}
        </div>

        <div className="flex justify-end gap-2 mt-5">
          <button onClick={onClose}
            className={`px-4 py-2 text-sm rounded transition ${theme === "light" ? "text-zinc-500 hover:text-zinc-700" : "text-zinc-400 hover:text-white"}`}>
            {t("modal.cancel")}
          </button>
          <button onClick={() => onSave(form)}
            className="px-4 py-2 text-sm bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition font-medium">
            {t("modal.save")}
          </button>
        </div>
      </div>
    </div>
  );
}

function Field({ label, value, onChange, type = "text", placeholder = "", theme, autoFocus = false }: {
  label: string; value: string; onChange: (v: string) => void; type?: string; placeholder?: string; theme: string; autoFocus?: boolean;
}) {
  return (
    <label className="block">
      <span className="text-xs text-zinc-500 mb-1 block">{label}</span>
      <input type={type} value={value} onChange={e => onChange(e.target.value)} placeholder={placeholder} autoFocus={autoFocus}
        className={`w-full rounded-lg px-3 py-2 text-sm placeholder:text-zinc-400 focus:outline-none focus:border-blue-500 transition ${
          theme === "light"
            ? "bg-white border border-zinc-300 text-zinc-900"
            : "bg-zinc-800 border border-zinc-700 text-white placeholder:text-zinc-600"
        }`} />
    </label>
  );
}

export default AppShell;
