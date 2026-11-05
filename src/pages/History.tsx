import { useCallback, useEffect, useState } from "react";
import { api, errorMessage } from "../lib/api";
import { notify, useStore } from "../lib/store";
import { dayLabel, formatTime, hostOf } from "../lib/format";
import type { HistoryEntry } from "../lib/types";
import { Favicon, Icon } from "../components/Icon";
import { openUrl } from "../components/BookmarksBar";
import { confirmAction } from "../components/Overlays";

const PAGE = 150;

function groupByDay(entries: HistoryEntry[]) {
  const days: { day: string; items: HistoryEntry[] }[] = [];
  for (const e of entries) {
    const day = dayLabel(e.visitedAt);
    if (days.length && days[days.length - 1].day === day) days[days.length - 1].items.push(e);
    else days.push({ day, items: [e] });
  }
  return days;
}

export function HistoryPage() {
  const incognito = useStore((s) => s.incognito);
  return incognito ? <IncognitoHistory /> : <RegularHistory />;
}

function IncognitoHistory() {
  return (
    <div className="page">
      <div className="page-inner">
        <div className="page-head">
          <h1>History</h1>
        </div>
        <div className="empty-state">
          <div className="icon">
            <Icon name="incognito" />
          </div>
          <div>History is off in incognito windows. Pages you visit here are never recorded, and your regular history is not shown.</div>
        </div>
      </div>
    </div>
  );
}

function RegularHistory() {
  const [query, setQuery] = useState("");
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [icons, setIcons] = useState<Record<string, string>>({});
  const [more, setMore] = useState(false);

  const load = useCallback(
    async (append: boolean) => {
      const before = append && entries.length ? entries[entries.length - 1].visitedAt : null;
      const page = await api.historySearch(query, before, PAGE);
      setEntries((prev) => (append ? [...prev, ...page] : page));
      setMore(page.length === PAGE);
      const hosts = [...new Set(page.map((e) => hostOf(e.url)))];
      api.favicons(hosts).then((f) => setIcons((prev) => ({ ...prev, ...f }))).catch(() => {});
    },
    [query, entries],
  );

  useEffect(() => {
    const t = window.setTimeout(() => load(false).catch((e) => notify(errorMessage(e), "error")), query ? 120 : 0);
    return () => window.clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query]);

  const remove = (id: number) =>
    api
      .historyDelete(id)
      .then(() => setEntries((e) => e.filter((x) => x.id !== id)))
      .catch((e) => notify(errorMessage(e), "error"));

  const clearAll = () =>
    confirmAction("Clear all history?", "Every page in your Zuno history will be removed. This cannot be undone.", "Clear history", () =>
      api
        .historyClear()
        .then(() => {
          setEntries([]);
          notify("History cleared");
        })
        .catch((e) => notify(errorMessage(e), "error")),
    );

  return (
    <div className="page">
      <div className="page-inner">
        <div className="page-head">
          <h1>History</h1>
          <span className="spacer" />
          <label className="search-field">
            <Icon name="search" size={14} />
            <input value={query} placeholder="Search history" aria-label="Search history" onChange={(e) => setQuery(e.target.value)} />
          </label>
          <button className="btn" onClick={clearAll} disabled={entries.length === 0 && !query}>
            Clear all
          </button>
        </div>
        {entries.length === 0 && (
          <div className="empty-state">
            <div className="icon">
              <Icon name="history" />
            </div>
            <div>{query ? "No history matches your search" : "Pages you visit will appear here"}</div>
          </div>
        )}
        {groupByDay(entries).map(({ day, items }) => (
          <div key={day}>
            <div className="day-label">{day}</div>
            <div className="list">
              {items.map((e) => (
                <div
                  key={e.id}
                  className="list-row clickable"
                  onClick={(ev) => openUrl(e.url, ev.ctrlKey)}
                  onAuxClick={(ev) => ev.button === 1 && openUrl(e.url, true)}
                >
                  <span className="time">{formatTime(e.visitedAt)}</span>
                  <Favicon src={icons[hostOf(e.url)]} />
                  <div className="main">
                    <div className="t">{e.title || e.url}</div>
                    <div className="u">{hostOf(e.url)}</div>
                  </div>
                  <div className="row-actions">
                    <button
                      className="icon-btn small"
                      aria-label="Remove from history"
                      title="Remove from history"
                      onClick={(ev) => {
                        ev.stopPropagation();
                        remove(e.id);
                      }}
                    >
                      <Icon name="close" size={14} />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        ))}
        {more && (
          <div style={{ textAlign: "center", marginTop: 20 }}>
            <button className="btn" onClick={() => load(true).catch((e) => notify(errorMessage(e), "error"))}>
              Show more
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
