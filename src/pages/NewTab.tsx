import { useEffect, useState } from "react";
import { api, errorMessage } from "../lib/api";
import { activeTab, getState, notify, setState, useStore } from "../lib/store";
import { hostOf } from "../lib/format";
import type { HistoryEntry } from "../lib/types";
import { Favicon, Icon, Logo } from "../components/Icon";

export function NewTabPage() {
  const engines = useStore((s) => s.engines);
  const engineId = useStore((s) => s.settings?.searchEngine);
  const incognito = useStore((s) => s.incognito);
  const [query, setQuery] = useState("");
  const [sites, setSites] = useState<HistoryEntry[]>([]);
  const [icons, setIcons] = useState<Record<string, string>>({});
  const engine = engines.find((e) => e.id === engineId)?.name ?? "the web";

  useEffect(() => {
    // Incognito windows never show sites from the regular history.
    if (incognito) return;
    api
      .topSites(8)
      .then((s) => {
        setSites(s);
        api.favicons(s.map((x) => hostOf(x.url))).then(setIcons).catch(() => {});
      })
      .catch(() => {});
  }, [incognito]);

  const go = (input: string) => {
    const tab = activeTab(getState());
    if (!tab || !input.trim()) return;
    api
      .navigate(tab.id, input)
      .then((outcome) => {
        if (outcome.kind === "authReview") setState({ overlay: { kind: "auth", tab: tab.id, request: outcome.request } });
      })
      .catch((e) => notify(errorMessage(e), "error"));
  };

  return (
    <div className="page">
      <div className="newtab">
        <div className="brand">
          {incognito ? <Icon name="incognito" size={30} /> : <Logo size={30} />}
          {incognito ? "Incognito" : "Zuno"}
        </div>
        <div className="tagline">{incognito ? "Nothing you do here is saved." : "Less noise. More web."}</div>
        <form
          className="search"
          onSubmit={(e) => {
            e.preventDefault();
            go(query);
          }}
        >
          <Icon name="search" />
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={`Search ${engine} or enter address`} aria-label="Search or enter address" spellCheck={false} />
        </form>
        {incognito && (
          <div className="incognito-info">
            <div>
              <h3>Not kept after you close this window</h3>
              <ul>
                <li>Pages you visit, searches and site icons</li>
                <li>Cookies, logins and site data</li>
                <li>Cache, form entries and the download list</li>
                <li>Which host names were looked up (DNS cache)</li>
              </ul>
            </div>
            <div>
              <h3>Still visible to others</h3>
              <ul>
                <li>Websites you visit, your employer or school, and your internet provider</li>
                <li>Files you download (where the file came from is removed from them)</li>
                <li>Your device's MAC address on the local network, unless MAC randomization is on in Settings</li>
              </ul>
            </div>
          </div>
        )}
        {sites.length > 0 && (
          <div className="tiles">
            {sites.map((s) => (
              <button key={s.url} className="tile" title={`${s.title}\n${s.url}`} onClick={() => go(s.url)}>
                <span className="ic">
                  <Favicon src={icons[hostOf(s.url)]} size={20} />
                </span>
                <span className="label">{s.title || hostOf(s.url).replace(/^www\./, "")}</span>
              </button>
            ))}
          </div>
        )}
        <div className="hint">
          <kbd>Ctrl</kbd> <kbd>K</kbd> to switch between tabs, bookmarks{incognito ? "" : " and history"}
        </div>
      </div>
    </div>
  );
}
