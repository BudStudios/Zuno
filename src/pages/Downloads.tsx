import { useEffect } from "react";
import { api, errorMessage } from "../lib/api";
import { notify, setState, useStore } from "../lib/store";
import { formatBytes, hostOf, relativeDate } from "../lib/format";
import type { DownloadItem } from "../lib/types";
import { Icon } from "../components/Icon";
import { openUrl } from "../components/BookmarksBar";

const act = (id: number, action: Parameters<typeof api.downloadAction>[1]) =>
  api.downloadAction(id, action).catch((e) => notify(errorMessage(e), "error"));

function status(d: DownloadItem): { text: string; error?: boolean } {
  const size = d.totalBytes ? `${formatBytes(d.receivedBytes)} of ${formatBytes(d.totalBytes)}` : formatBytes(d.receivedBytes);
  switch (d.state) {
    case "inprogress":
      return { text: size || "Starting…" };
    case "paused":
      return { text: d.error ? `${d.error} — ${size}` : `Paused — ${size}` };
    case "completed":
      return d.exists ? { text: `${formatBytes(d.totalBytes ?? d.receivedBytes)} · ${hostOf(d.url)}` } : { text: "Deleted or moved", error: true };
    case "cancelled":
      return { text: "Cancelled" };
    case "failed":
      return { text: d.error ?? "Failed", error: true };
  }
}

function Row({ d }: { d: DownloadItem }) {
  const s = status(d);
  const active = d.state === "inprogress" || d.state === "paused";
  const pct = d.totalBytes ? Math.min(100, (d.receivedBytes / d.totalBytes) * 100) : null;
  return (
    <div className="list-row">
      <Icon name="file" />
      <div className="main">
        <div className="t" title={d.path}>
          {d.state === "completed" && d.exists ? (
            <a href="#" onClick={(e) => (e.preventDefault(), act(d.id, "open"))}>
              {d.fileName}
            </a>
          ) : (
            <span style={d.state === "cancelled" || d.state === "failed" ? { textDecoration: "line-through", opacity: 0.7 } : undefined}>{d.fileName}</span>
          )}
        </div>
        <div className={`status-text${s.error ? " error" : ""}`}>{s.text}</div>
        {active && (
          <div className={`progress${pct == null ? " indeterminate" : ""}`}>
            <div style={pct == null ? undefined : { width: `${pct}%` }} />
          </div>
        )}
      </div>
      <div className="row-actions" style={active ? { opacity: 1 } : undefined}>
        {d.state === "inprogress" && (
          <button className="icon-btn small" title="Pause" aria-label="Pause" onClick={() => act(d.id, "pause")}>
            <Icon name="pause" size={14} />
          </button>
        )}
        {d.state === "paused" && d.canResume && (
          <button className="icon-btn small" title="Resume" aria-label="Resume" onClick={() => act(d.id, "resume")}>
            <Icon name="play" size={14} />
          </button>
        )}
        {active && (
          <button className="icon-btn small" title="Cancel" aria-label="Cancel" onClick={() => act(d.id, "cancel")}>
            <Icon name="close" size={14} />
          </button>
        )}
        {d.state === "completed" && d.exists && (
          <button className="icon-btn small" title="Show in folder" aria-label="Show in folder" onClick={() => act(d.id, "show")}>
            <Icon name="folder" size={14} />
          </button>
        )}
        {(d.state === "failed" || d.state === "cancelled") && (
          <button className="icon-btn small" title="Retry" aria-label="Retry" onClick={() => openUrl(d.url, true)}>
            <Icon name="reload" size={14} />
          </button>
        )}
        {!active && (
          <button
            className="icon-btn small"
            title="Remove from list"
            aria-label="Remove from list"
            onClick={() =>
              api
                .downloadAction(d.id, "remove")
                .then(() => setState((s) => ({ downloads: s.downloads.filter((x) => x.id !== d.id) })))
                .catch((e) => notify(errorMessage(e), "error"))
            }
          >
            <Icon name="trash" size={14} />
          </button>
        )}
      </div>
    </div>
  );
}

export function DownloadsPage() {
  const downloads = useStore((s) => s.downloads);
  useEffect(() => {
    api.downloads().then((downloads) => setState({ downloads })).catch(() => {});
  }, []);
  const days: { day: string; items: DownloadItem[] }[] = [];
  for (const d of downloads) {
    const day = relativeDate(d.startedAt);
    if (days.length && days[days.length - 1].day === day) days[days.length - 1].items.push(d);
    else days.push({ day, items: [d] });
  }
  return (
    <div className="page">
      <div className="page-inner">
        <div className="page-head">
          <h1>Downloads</h1>
          <span className="spacer" />
          <button className="btn" onClick={() => api.downloadsOpenFolder().catch((e) => notify(errorMessage(e), "error"))}>
            <Icon name="folder" size={14} /> Open folder
          </button>
          <button
            className="btn"
            disabled={!downloads.some((d) => d.state !== "inprogress" && d.state !== "paused")}
            onClick={() =>
              api
                .downloadsClear()
                .then(() => setState((s) => ({ downloads: s.downloads.filter((d) => d.state === "inprogress" || d.state === "paused") })))
                .catch((e) => notify(errorMessage(e), "error"))
            }
          >
            Clear list
          </button>
        </div>
        {downloads.length === 0 ? (
          <div className="empty-state">
            <div className="icon">
              <Icon name="download" />
            </div>
            <div>Files you download will appear here</div>
          </div>
        ) : (
          days.map(({ day, items }) => (
            <div key={day}>
              <div className="day-label">{day}</div>
              <div className="list">
                {items.map((d) => (
                  <Row key={d.id} d={d} />
                ))}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
