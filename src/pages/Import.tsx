import { useEffect, useState } from "react";
import { api, errorMessage, on } from "../lib/api";
import { bookmarksChanged, notify } from "../lib/store";
import type { BrowserSource, CategoryResult, ImportCategory, ImportProgress, Inspection } from "../lib/types";
import { Icon, Spinner } from "../components/Icon";

const LABELS: Record<ImportCategory, string> = {
  bookmarks: "Bookmarks",
  history: "Browsing history",
  cookies: "Cookies (site sign-ins)",
  passwords: "Saved passwords",
  settings: "Settings",
};

type Step = "source" | "categories" | "confirm" | "running" | "done";

export function ImportPage() {
  const [step, setStep] = useState<Step>("source");
  const [sources, setSources] = useState<BrowserSource[] | null>(null);
  const [source, setSource] = useState<string>("");
  const [profile, setProfile] = useState<string>("");
  const [inspection, setInspection] = useState<Inspection | null>(null);
  const [inspecting, setInspecting] = useState(false);
  const [selected, setSelected] = useState<ImportCategory[]>([]);
  const [progress, setProgress] = useState<ImportProgress | null>(null);
  const [results, setResults] = useState<CategoryResult[]>([]);

  useEffect(() => {
    api
      .importDetect()
      .then((s) => {
        setSources(s);
        if (s[0]) {
          setSource(s[0].id);
          setProfile(s[0].profiles[0]?.id ?? "");
        }
      })
      .catch((e) => notify(errorMessage(e), "error"));
    return on<ImportProgress>("zuno://import-progress", setProgress);
  }, []);

  const current = sources?.find((s) => s.id === source);

  const inspect = async () => {
    setInspecting(true);
    try {
      const result = await api.importInspect(source, profile);
      setInspection(result);
      setSelected(result.categories.filter((c) => c.available && c.category !== "cookies" && c.category !== "settings").map((c) => c.category));
      setStep("categories");
    } catch (e) {
      notify(errorMessage(e), "error");
    } finally {
      setInspecting(false);
    }
  };

  const run = async (confirmSettingsOverwrite: boolean) => {
    setStep("running");
    try {
      const r = await api.importRun({ source, profile, categories: selected, confirmSettingsOverwrite });
      setResults(r);
      bookmarksChanged();
      setStep("done");
    } catch (e) {
      notify(errorMessage(e), "error");
      setStep("categories");
    }
  };

  const next = () => {
    const wantsSettings = selected.includes("settings") && (inspection?.settingChanges.length ?? 0) > 0;
    if (wantsSettings) setStep("confirm");
    else run(false);
  };

  const stepNames: [Step[], string][] = [
    [["source"], "1. Browser"],
    [["categories", "confirm"], "2. Data"],
    [["running", "done"], "3. Import"],
  ];

  return (
    <div className="page">
      <div className="page-inner" style={{ maxWidth: 680 }}>
        <div className="page-head">
          <h1>Import browser data</h1>
        </div>
        <div className="steps">
          {stepNames.map(([steps, label], i) => (
            <span key={label} className={steps.includes(step) ? "on" : ""}>
              {label}
              {i < stepNames.length - 1 && <span className="muted"> — </span>}
            </span>
          ))}
        </div>

        {step === "source" && (
          <>
            {sources === null && (
              <div className="empty-state">
                <Spinner /> Looking for browsers…
              </div>
            )}
            {sources?.length === 0 && (
              <div className="empty-state">
                <div className="icon">
                  <Icon name="import" />
                </div>
                <div>No supported browsers were found for this Windows user.</div>
              </div>
            )}
            {sources && sources.length > 0 && (
              <>
                <div className="list">
                  {sources.map((s) => (
                    <button
                      key={s.id}
                      className="choice"
                      role="radio"
                      aria-checked={source === s.id}
                      onClick={() => {
                        setSource(s.id);
                        setProfile(s.profiles[0]?.id ?? "");
                      }}
                    >
                      <span className={`radio${source === s.id ? " on" : ""}`} />
                      <span className="main">
                        <div>{s.name}</div>
                        <div className="d">
                          {s.profiles.length} profile{s.profiles.length === 1 ? "" : "s"}
                        </div>
                      </span>
                    </button>
                  ))}
                </div>
                {current && current.profiles.length > 1 && (
                  <div className="list">
                    <div className="setting">
                      <div className="label">Profile</div>
                      <select className="field" value={profile} aria-label="Profile" onChange={(e) => setProfile(e.target.value)}>
                        {current.profiles.map((p) => (
                          <option key={p.id} value={p.id}>
                            {p.name}
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>
                )}
                <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 20 }}>
                  <button className="btn primary" disabled={!source || !profile || inspecting} onClick={inspect}>
                    {inspecting ? "Checking…" : "Continue"}
                  </button>
                </div>
              </>
            )}
          </>
        )}

        {step === "categories" && inspection && (
          <>
            <p className="muted" style={{ marginTop: 0 }}>
              Choose what to import from {current?.name}. Zuno reads a copy of the data and never changes the other browser. Nothing in Zuno is replaced — imported bookmarks go into their
              own folder.
            </p>
            <div className="list">
              {inspection.categories.map((c) => {
                const on = selected.includes(c.category);
                return (
                  <button
                    key={c.category}
                    className="choice"
                    disabled={!c.available}
                    role="checkbox"
                    aria-checked={on}
                    onClick={() => setSelected((s) => (on ? s.filter((x) => x !== c.category) : [...s, c.category]))}
                  >
                    <span className={`checkbox${on ? " on" : ""}${c.available ? "" : " disabled"}`}>{on && <Icon name="check" size={12} />}</span>
                    <span className="main">
                      <div style={c.available ? undefined : { color: "var(--text-2)" }}>{LABELS[c.category]}</div>
                      {c.reason && <div className="d">{c.reason}</div>}
                    </span>
                    {c.available && c.count != null && (
                      <span className="pill">{c.category === "settings" ? `${c.count} change${c.count === 1 ? "" : "s"}` : c.count.toLocaleString()}</span>
                    )}
                    {!c.available && <span className="pill">Not available</span>}
                  </button>
                );
              })}
            </div>
            <div style={{ display: "flex", justifyContent: "space-between", marginTop: 20 }}>
              <button className="btn ghost" onClick={() => setStep("source")}>
                Back
              </button>
              <button className="btn primary" disabled={selected.length === 0} onClick={next}>
                Import
              </button>
            </div>
          </>
        )}

        {step === "confirm" && inspection && (
          <>
            <p style={{ marginTop: 0 }}>Importing settings will replace these Zuno settings:</p>
            <div className="list">
              {inspection.settingChanges.map((c) => (
                <div className="setting" key={c.key}>
                  <div className="label">
                    <div>{c.label}</div>
                    <div className="d">
                      {c.current} → <strong style={{ color: "var(--text)" }}>{c.imported}</strong>
                    </div>
                  </div>
                </div>
              ))}
            </div>
            <div style={{ display: "flex", justifyContent: "space-between", marginTop: 20 }}>
              <button className="btn ghost" onClick={() => setStep("categories")}>
                Back
              </button>
              <div style={{ display: "flex", gap: 8 }}>
                <button className="btn" onClick={() => run(false)}>
                  Keep my settings
                </button>
                <button className="btn primary" onClick={() => run(true)}>
                  Replace settings
                </button>
              </div>
            </div>
          </>
        )}

        {step === "running" && (
          <div className="list">
            <div className="setting">
              <Spinner />
              <div className="label">
                <div>{progress ? `${LABELS[progress.category]} — ${progress.message}` : "Starting…"}</div>
                {progress && progress.total > 0 && (
                  <div className="progress">
                    <div style={{ width: `${Math.round((progress.done / progress.total) * 100)}%` }} />
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {step === "done" && (
          <>
            <div className="list">
              {results.map((r) => (
                <div className="setting" key={r.category}>
                  <span className={r.status === "imported" ? "pill ok" : r.status === "failed" ? "pill bad" : "pill"}>
                    {r.status === "imported" ? "Imported" : r.status === "failed" ? "Failed" : r.status === "skipped" ? "Skipped" : "Not available"}
                  </span>
                  <div className="label">
                    <div>
                      {LABELS[r.category]}
                      {r.status === "imported" && ` · ${r.count.toLocaleString()}`}
                    </div>
                    {r.message && <div className="d">{r.message}</div>}
                  </div>
                </div>
              ))}
            </div>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 20 }}>
              <button
                className="btn"
                onClick={() => {
                  setStep("source");
                  setResults([]);
                  setProgress(null);
                }}
              >
                Import more
              </button>
              <button className="btn primary" onClick={() => api.openPage("bookmarks")}>
                View bookmarks
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
