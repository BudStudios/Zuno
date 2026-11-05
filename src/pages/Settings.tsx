import { useEffect, useState } from "react";
import { api, errorMessage } from "../lib/api";
import { notify, setState, useStore } from "../lib/store";
import type { Account, AppInfo, Settings, Startup, Theme, TrackingPrevention } from "../lib/types";
import { Icon, type IconName } from "../components/Icon";
import { confirmAction } from "../components/Overlays";

const fail = (e: unknown) => notify(errorMessage(e), "error");

type Section = "appearance" | "search" | "privacy" | "downloads" | "startup" | "accounts" | "import" | "about";

const SECTIONS: { id: Section; label: string; icon: IconName }[] = [
  { id: "appearance", label: "Appearance", icon: "sun" },
  { id: "search", label: "Search engine", icon: "search" },
  { id: "privacy", label: "Privacy", icon: "shield" },
  { id: "downloads", label: "Downloads", icon: "download" },
  { id: "startup", label: "Startup", icon: "home" },
  { id: "accounts", label: "Accounts", icon: "key" },
  { id: "import", label: "Import and export", icon: "import" },
  { id: "about", label: "About Zuno", icon: "info" },
];

const INCOGNITO_HIDDEN: Section[] = ["startup", "accounts", "import"];

function Switch({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return <button className={`switch${on ? " on" : ""}`} role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)} />;
}

function Row({ title, desc, children }: { title: string; desc?: React.ReactNode; children?: React.ReactNode }) {
  return (
    <div className="setting">
      <div className="label">
        <div>{title}</div>
        {desc && <div className="d">{desc}</div>}
      </div>
      {children}
    </div>
  );
}

function TextSetting({ value, placeholder, onSave, width = 300 }: { value: string; placeholder?: string; onSave: (v: string) => void; width?: number }) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  return (
    <input
      className="field"
      style={{ width }}
      value={draft}
      placeholder={placeholder}
      spellCheck={false}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => draft !== value && onSave(draft)}
      onKeyDown={(e) => e.key === "Enter" && (e.currentTarget as HTMLInputElement).blur()}
    />
  );
}

function Appearance({ s, update }: { s: Settings; update: (p: Partial<Settings>) => void }) {
  const themes: { id: Theme; label: string; icon: IconName }[] = [
    { id: "light", label: "Light", icon: "sun" },
    { id: "dark", label: "Dark", icon: "moon" },
    { id: "system", label: "System", icon: "monitor" },
  ];
  return (
    <section>
      <h2>Appearance</h2>
      <div className="list">
        <Row title="Theme" desc="Websites that support it follow this choice too.">
          <div className="segmented" role="radiogroup" aria-label="Theme">
            {themes.map((t) => (
              <button key={t.id} role="radio" aria-checked={s.theme === t.id} className={s.theme === t.id ? "on" : ""} onClick={() => update({ theme: t.id })}>
                <Icon name={t.icon} size={14} /> {t.label}
              </button>
            ))}
          </div>
        </Row>
        <Row title="Show bookmarks bar" desc="Ctrl+Shift+B">
          <Switch label="Show bookmarks bar" on={s.showBookmarksBar} onChange={(v) => update({ showBookmarksBar: v })} />
        </Row>
      </div>
    </section>
  );
}

function Search({ s, update }: { s: Settings; update: (p: Partial<Settings>) => void }) {
  const engines = useStore((x) => x.engines);
  return (
    <section>
      <h2>Search engine</h2>
      <div className="list">
        <Row title="Search engine used in the address bar">
          <select className="field" value={s.searchEngine} aria-label="Search engine" onChange={(e) => update({ searchEngine: e.target.value })}>
            {engines.map((e) => (
              <option key={e.id} value={e.id}>
                {e.name}
              </option>
            ))}
            <option value="custom" disabled={!s.customSearchUrl}>
              Custom{s.customSearchUrl ? "" : " (set an address below)"}
            </option>
          </select>
        </Row>
        <Row title="Custom search address" desc="Use %s where the search terms go, e.g. https://search.example/?q=%s">
          <TextSetting
            value={s.customSearchUrl}
            placeholder="https://…?q=%s"
            onSave={(v) => update({ customSearchUrl: v, searchEngine: v ? "custom" : s.searchEngine === "custom" ? "duckduckgo" : s.searchEngine })}
          />
        </Row>
      </div>
    </section>
  );
}

function Privacy({ s, update }: { s: Settings; update: (p: Partial<Settings>) => void }) {
  const levels: { id: TrackingPrevention; label: string }[] = [
    { id: "none", label: "Off" },
    { id: "basic", label: "Basic" },
    { id: "balanced", label: "Balanced" },
    { id: "strict", label: "Strict" },
  ];
  const incognito = useStore((st) => st.incognito);
  const [clear, setClear] = useState({ history: true, downloads: false, cookies: false, cache: true, siteStorage: false, passwords: false, autofill: false });
  const [busy, setBusy] = useState(false);
  const toggle = (k: keyof typeof clear) => setClear((c) => ({ ...c, [k]: !c[k] }));
  const items: [keyof typeof clear, string, string][] = [
    ["history", "Browsing history", "Pages you visited and their icons"],
    ["downloads", "Download list", "Files stay on disk"],
    ["cookies", "Cookies", "Signs you out of most sites"],
    ["siteStorage", "Site storage", "Local storage, IndexedDB and other site data"],
    ["cache", "Cached images and files", "Pages may load more slowly next time"],
    ["passwords", "Saved passwords", "Passwords saved by WebView2's password manager"],
    ["autofill", "Autofill data", "Saved form entries"],
  ];
  const runClear = () =>
    confirmAction("Clear browsing data?", "The selected data will be permanently deleted from this computer.", "Clear data", () => {
      setBusy(true);
      api
        .clearBrowsingData(clear)
        .then(() => notify("Browsing data cleared"))
        .catch(fail)
        .finally(() => setBusy(false));
    });

  return (
    <>
      <section>
        <h2>Privacy</h2>
        <div className="list">
          <Row title="Tracking prevention" desc="Blocks known trackers using Microsoft Edge's tracking prevention lists. Strict may break some sites.">
            <div className="segmented" role="radiogroup" aria-label="Tracking prevention">
              {levels.map((l) => (
                <button key={l.id} role="radio" aria-checked={s.trackingPrevention === l.id} className={s.trackingPrevention === l.id ? "on" : ""} onClick={() => update({ trackingPrevention: l.id })}>
                  {l.label}
                </button>
              ))}
            </div>
          </Row>
          {!incognito && (
            <>
              <Row title="Save browsing history" desc="Sign-in codes and tokens in addresses are never recorded.">
                <Switch label="Save browsing history" on={s.saveHistory} onChange={(v) => update({ saveHistory: v })} />
              </Row>
              <Row title="Clear history when Zuno closes">
                <Switch label="Clear history when Zuno closes" on={s.clearHistoryOnExit} onChange={(v) => update({ clearHistoryOnExit: v })} />
              </Row>
              <Row title="Offer to save passwords" desc="Handled by WebView2's built-in password manager and stored encrypted in your Zuno profile.">
                <Switch label="Offer to save passwords" on={s.passwordAutosave} onChange={(v) => update({ passwordAutosave: v })} />
              </Row>
              <Row title="Autofill forms">
                <Switch label="Autofill forms" on={s.generalAutofill} onChange={(v) => update({ generalAutofill: v })} />
              </Row>
            </>
          )}
        </div>
      </section>
      <section>
        <h2>Incognito windows</h2>
        <div className="list">
          <Row
            title="Randomize MAC address"
            desc="While incognito windows are open, active network adapters use a random hardware address, and the original returns when the last one closes. Your local network (router, hotspot) sees the random address; websites never see a MAC address either way. Needs administrator approval and reconnects the network for a few seconds. Some adapter drivers don't allow it."
          >
            <Switch label="Randomize MAC address" on={s.incognitoRandomizeMac} onChange={(v) => update({ incognitoRandomizeMac: v })} />
          </Row>
          {!incognito && (
            <div className="setting" style={{ justifyContent: "flex-end" }}>
              <button className="btn" onClick={() => api.openIncognito().catch(fail)}>
                <Icon name="incognito" size={14} />
                New incognito window
              </button>
            </div>
          )}
        </div>
      </section>
      <section>
        <h2>Clear browsing data</h2>
        <div className="list">
          {items.map(([k, label, desc]) => (
            <button key={k} className="choice" onClick={() => toggle(k)} role="checkbox" aria-checked={clear[k]}>
              <span className={`checkbox${clear[k] ? " on" : ""}`}>{clear[k] && <Icon name="check" size={12} />}</span>
              <span className="main">
                <div>{label}</div>
                <div className="d">{desc}</div>
              </span>
            </button>
          ))}
          <div className="setting" style={{ justifyContent: "flex-end" }}>
            <button className="btn" disabled={busy || !Object.values(clear).some(Boolean)} onClick={runClear}>
              {busy ? "Clearing…" : "Clear data"}
            </button>
          </div>
        </div>
      </section>
    </>
  );
}

function Downloads({ s, update }: { s: Settings; update: (p: Partial<Settings>) => void }) {
  return (
    <section>
      <h2>Downloads</h2>
      <div className="list">
        <Row title="Download location" desc={<span className="mono">{s.downloadDir ?? "Downloads folder"}</span>}>
          <div style={{ display: "flex", gap: 6 }}>
            {s.downloadDir && (
              <button className="btn ghost" onClick={() => update({ downloadDir: null })}>
                Reset
              </button>
            )}
            <button
              className="btn"
              onClick={() =>
                api
                  .chooseDownloadDir()
                  .then((dir) => dir && update({ downloadDir: dir }))
                  .catch(fail)
              }
            >
              Change…
            </button>
          </div>
        </Row>
        <Row title="Existing files are never overwritten" desc="If a file with the same name exists, Zuno saves the new one as “name (1)”.">
          <button className="btn" onClick={() => api.downloadsOpenFolder().catch(fail)}>
            Open folder
          </button>
        </Row>
      </div>
    </section>
  );
}

function StartupSection({ s, update }: { s: Settings; update: (p: Partial<Settings>) => void }) {
  const options: { id: Startup; label: string }[] = [
    { id: "restore", label: "Continue where you left off" },
    { id: "newtab", label: "Open a new tab" },
    { id: "homepage", label: "Open the homepage" },
  ];
  return (
    <section>
      <h2>Startup</h2>
      <div className="list">
        {options.map((o) => (
          <button key={o.id} className="choice" role="radio" aria-checked={s.startup === o.id} onClick={() => update({ startup: o.id })}>
            <span className={`radio${s.startup === o.id ? " on" : ""}`} />
            <span className="main">{o.label}</span>
          </button>
        ))}
        <Row title="Homepage" desc="Leave empty to use the new tab page.">
          <TextSetting value={s.homepage === "zuno://newtab" ? "" : s.homepage} placeholder="https://" onSave={(v) => update({ homepage: v })} />
        </Row>
      </div>
    </section>
  );
}

function Accounts() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [form, setForm] = useState({ issuer: "", clientId: "", clientSecret: "", scopes: "openid profile email" });
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const load = () => api.accounts().then(setAccounts).catch(fail);
  useEffect(() => {
    load();
  }, []);
  return (
    <>
      <section>
        <h2>How Zuno handles sign-in</h2>
        <div className="list">
          <Row title="Website sign-ins" desc="Sites keep you signed in with their own cookies inside your Zuno profile. Zuno never reads or exports them." />
          <Row
            title="Pasted authorization links"
            desc="When you paste an OAuth or OpenID Connect sign-in link, Zuno shows who is asking and where the result goes before continuing. You always approve on the provider's own page."
          />
          <Row title="Accounts connected to Zuno" desc="Use the standard authorization code flow with PKCE. Tokens are kept in Windows Credential Manager, never in plain files." />
        </div>
      </section>
      <section>
        <h2>Connected accounts</h2>
        <div className="list">
          {accounts.length === 0 && <Row title="No accounts connected" desc="Connect an OpenID Connect provider you have registered Zuno with (public client, loopback redirect)." />}
          {accounts.map((a) => (
            <Row key={a.id} title={a.email ?? a.displayName ?? a.subject ?? "Account"} desc={`${a.issuer} · ${a.scopes}`}>
              <button
                className="btn"
                onClick={() =>
                  confirmAction("Disconnect account?", "Zuno will delete its stored tokens and ask the provider to revoke them.", "Disconnect", () =>
                    api.accountDisconnect(a.id).then(load).catch(fail),
                  )
                }
              >
                Disconnect
              </button>
            </Row>
          ))}
          {!open ? (
            <div className="setting" style={{ justifyContent: "flex-end" }}>
              <button className="btn" onClick={() => setOpen(true)}>
                <Icon name="plus" size={14} /> Connect account
              </button>
            </div>
          ) : (
            <form
              className="setting stack"
              onSubmit={(e) => {
                e.preventDefault();
                setBusy(true);
                api
                  .accountConnect(form.issuer, form.clientId, form.clientSecret || null, form.scopes)
                  .then(() => setOpen(false))
                  .catch(fail)
                  .finally(() => setBusy(false));
              }}
            >
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
                <input className="field" required placeholder="Issuer, e.g. https://accounts.google.com" value={form.issuer} onChange={(e) => setForm({ ...form, issuer: e.target.value })} />
                <input className="field" required placeholder="Client ID" value={form.clientId} onChange={(e) => setForm({ ...form, clientId: e.target.value })} />
                <input className="field" type="password" placeholder="Client secret (only if the provider requires one)" value={form.clientSecret} onChange={(e) => setForm({ ...form, clientSecret: e.target.value })} />
                <input className="field" placeholder="Scopes" value={form.scopes} onChange={(e) => setForm({ ...form, scopes: e.target.value })} />
              </div>
              <div className="d muted" style={{ fontSize: 12 }}>
                Redirect URI to register: <span className="mono">http://127.0.0.1/zuno/oauth/callback</span> (any port). A sign-in tab opens; the redirect is handled inside Zuno.
              </div>
              <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
                <button type="button" className="btn ghost" onClick={() => setOpen(false)}>
                  Cancel
                </button>
                <button type="submit" className="btn primary" disabled={busy}>
                  {busy ? "Contacting provider…" : "Continue"}
                </button>
              </div>
            </form>
          )}
        </div>
      </section>
    </>
  );
}

function ImportExport() {
  return (
    <section>
      <h2>Import and export</h2>
      <div className="list">
        <Row title="Import from another browser" desc="Bookmarks, history and settings from Chrome, Edge, Firefox and other browsers.">
          <button className="btn" onClick={() => api.openPage("import").catch(fail)}>
            Import…
          </button>
        </Row>
        <Row title="Export bookmarks" desc="Saves an HTML file that any browser can import.">
          <button
            className="btn"
            onClick={() =>
              api
                .bookmarksExport()
                .then((p) => p && notify(`Bookmarks exported to ${p}`))
                .catch(fail)
            }
          >
            Export…
          </button>
        </Row>
      </div>
    </section>
  );
}

function About() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => {
    api.appInfo().then(setInfo).catch(fail);
  }, []);
  return (
    <section>
      <h2>About Zuno</h2>
      <div className="list">
        <Row title="Zuno" desc="Less noise. More web.">
          <span className="muted">Version {info?.version}</span>
        </Row>
        <Row title="Rendering engine" desc="Websites are rendered by the Microsoft Edge WebView2 runtime installed on this computer, which updates automatically.">
          <span className="muted">{info?.webviewVersion}</span>
        </Row>
        <Row title="Application framework">
          <span className="muted">Tauri {info?.tauriVersion}</span>
        </Row>
        <Row title="Profile folder" desc={<span className="mono">{info?.dataDir}</span>} />
      </div>
    </section>
  );
}

export function SettingsPage() {
  const settings = useStore((s) => s.settings);
  const incognito = useStore((s) => s.incognito);
  const [section, setSection] = useState<Section>("appearance");
  if (!settings) return null;
  // Sections that only make sense for the regular profile.
  const sections = incognito ? SECTIONS.filter((s) => !INCOGNITO_HIDDEN.includes(s.id)) : SECTIONS;

  const update = (patch: Partial<Settings>) => {
    const next = { ...settings, ...patch };
    setState({ settings: next });
    api
      .setSettings(next)
      .then((saved) => setState({ settings: saved }))
      .catch((e) => {
        setState({ settings });
        fail(e);
      });
  };

  return (
    <div className="page">
      <div className="settings">
        <nav aria-label="Settings sections">
          <h1>Settings</h1>
          {sections.map((s) => (
            <button key={s.id} className={section === s.id ? "on" : ""} aria-current={section === s.id} onClick={() => setSection(s.id)}>
              <Icon name={s.icon} size={15} />
              {s.label}
            </button>
          ))}
        </nav>
        <main>
          {incognito && (
            <div className="incognito-note">
              <Icon name="incognito" />
              Changes made in an incognito window last only until it closes.
            </div>
          )}
          {section === "appearance" && <Appearance s={settings} update={update} />}
          {section === "search" && <Search s={settings} update={update} />}
          {section === "privacy" && <Privacy s={settings} update={update} />}
          {section === "downloads" && <Downloads s={settings} update={update} />}
          {section === "startup" && <StartupSection s={settings} update={update} />}
          {section === "accounts" && <Accounts />}
          {section === "import" && <ImportExport />}
          {section === "about" && <About />}
        </main>
      </div>
    </div>
  );
}
