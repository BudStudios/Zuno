import { useEffect, useRef, useState } from "react";
import { api, errorMessage } from "../lib/api";
import { activeTab, bookmarksChanged, getState, notify, setState, useStore } from "../lib/store";
import { displayUrl, isInternal, securityOf } from "../lib/format";
import type { Bookmark, Tab } from "../lib/types";
import { Icon, Logo } from "./Icon";

const run = (p: Promise<unknown>) => p.catch((e) => notify(errorMessage(e), "error"));
const BOOKMARKS_BAR = 1;

/** Ctrl+D / star button: bookmark the current page, or edit the existing bookmark. */
export async function toggleBookmark() {
  const tab = activeTab(getState());
  if (!tab || isInternal(tab.url)) return;
  try {
    let bookmark = await api.bookmarkFind(tab.url);
    if (!bookmark) {
      bookmark = await api.bookmarkAdd(BOOKMARKS_BAR, tab.title, tab.url);
      bookmarksChanged();
    }
    setState({ overlay: { kind: "bookmark", bookmarkId: bookmark.id } });
  } catch (e) {
    notify(errorMessage(e), "error");
  }
}

function useBookmark(url: string | undefined): Bookmark | null {
  const tick = useStore((s) => s.bookmarksTick);
  const [bookmark, setBookmark] = useState<Bookmark | null>(null);
  useEffect(() => {
    let alive = true;
    if (!url || isInternal(url)) {
      setBookmark(null);
      return;
    }
    api
      .bookmarkFind(url)
      .then((b) => alive && setBookmark(b))
      .catch(() => alive && setBookmark(null));
    return () => {
      alive = false;
    };
  }, [url, tick]);
  return bookmark;
}

const SECURITY_TEXT = {
  secure: "Connection is secure — HTTPS with a certificate verified by WebView2",
  insecure: "Not secure — information sent to this site could be read by others",
  internal: "Zuno page",
  file: "Local file",
  other: "Local or unencrypted address",
};

function AddressBar({ tab }: { tab: Tab | undefined }) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [focused, setFocused] = useState(false);
  const [value, setValue] = useState("");
  const focusTick = useStore((s) => s.focusAddressTick);
  const url = tab?.url ?? "";
  const isNewTab = url === "zuno://newtab";
  const security = securityOf(url);

  const editableValue = isNewTab ? "" : url;

  useEffect(() => {
    if (!focused) setValue(editableValue);
  }, [editableValue, focused]);

  // Focus the address bar on Ctrl+L and when a fresh new tab opens.
  useEffect(() => {
    if (focusTick > 0) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [focusTick]);
  useEffect(() => {
    if (isNewTab) requestAnimationFrame(() => inputRef.current?.focus());
  }, [tab?.id, isNewTab]);

  const submit = async (newTab: boolean) => {
    const text = value.trim();
    if (!text || !tab) return;
    try {
      if (newTab) {
        await api.newTab(text);
      } else {
        const outcome = await api.navigate(tab.id, text);
        if (outcome.kind === "authReview") {
          setState({ overlay: { kind: "auth", tab: tab.id, request: outcome.request } });
        }
      }
      inputRef.current?.blur();
      if (!isInternal(getState().tabs.find((t) => t.id === tab.id)?.url ?? "")) api.focusContent().catch(() => {});
    } catch (e) {
      notify(errorMessage(e), "error");
    }
  };

  const display = displayUrl(url);
  const showDisplay = !focused && !isNewTab && !isInternal(url) && display.host;

  return (
    <div className={`address${focused ? " focused" : ""}`} onMouseDown={(e) => {
      if (!focused && e.target === e.currentTarget) {
        e.preventDefault();
        inputRef.current?.focus();
      }
    }}>
      <span className={`security ${security}`} title={SECURITY_TEXT[security]}>
        {security === "secure" && <Icon name="lock" size={14} />}
        {security === "insecure" && <Icon name="warning" size={14} />}
        {security === "internal" && <Logo size={15} />}
        {security === "file" && <Icon name="file" size={14} />}
        {security === "other" && <Icon name={focused || isNewTab ? "search" : "globe"} size={14} />}
      </span>
      {showDisplay && (
        <span className="display" onMouseDown={(e) => {
          e.preventDefault();
          inputRef.current?.focus();
        }}>
          <span>{display.host}</span>
          <span className="rest">{display.rest}</span>
        </span>
      )}
      <input
        ref={inputRef}
        style={showDisplay ? { position: "absolute", opacity: 0, width: 0, pointerEvents: "none" } : undefined}
        value={value}
        placeholder="Search or enter address"
        spellCheck={false}
        autoComplete="off"
        aria-label="Address and search bar"
        onChange={(e) => setValue(e.target.value)}
        onFocus={(e) => {
          setFocused(true);
          setValue(editableValue);
          const el = e.currentTarget;
          requestAnimationFrame(() => el.select());
        }}
        onBlur={() => setFocused(false)}
        onKeyDown={(e) => {
          if (e.key === "Enter") submit(e.altKey);
          if (e.key === "Escape") {
            setValue(editableValue);
            e.currentTarget.blur();
            if (!isInternal(url)) api.focusContent().catch(() => {});
          }
        }}
      />
    </div>
  );
}

function DownloadButton() {
  const downloads = useStore((s) => s.downloads);
  const active = downloads.filter((d) => d.state === "inprogress" || d.state === "paused");
  const recent = downloads.some((d) => d.finishedAt && Date.now() - d.finishedAt < 60 * 60 * 1000);
  if (active.length === 0 && !recent) return null;
  const total = active.reduce((n, d) => n + (d.totalBytes ?? 0), 0);
  const received = active.reduce((n, d) => n + d.receivedBytes, 0);
  const fraction = total > 0 ? received / total : active.length ? 0.15 : 1;
  const r = 11;
  const c = 2 * Math.PI * r;
  return (
    <button
      className="icon-btn download-indicator"
      aria-label="Downloads"
      title={active.length ? `${active.length} download${active.length > 1 ? "s" : ""} in progress` : "Downloads (Ctrl+J)"}
      onClick={() => run(api.openPage("downloads"))}
    >
      <Icon name="download" />
      {active.length > 0 && (
        <svg className="ring" viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="12" cy="12" r={r} fill="none" stroke="currentColor" strokeOpacity="0.15" strokeWidth="1.5" />
          <circle
            cx="12"
            cy="12"
            r={r}
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeDasharray={`${c * fraction} ${c}`}
            transform="rotate(-90 12 12)"
          />
        </svg>
      )}
    </button>
  );
}

export function Toolbar() {
  const tab = useStore((s) => activeTab(s));
  const bookmark = useBookmark(tab?.url);
  const menuRef = useRef<HTMLButtonElement>(null);
  const internal = !tab || isInternal(tab.url);

  return (
    <div className="toolbar">
      <button
        className="icon-btn"
        aria-label="Back"
        title="Back (Alt+Left)"
        disabled={!tab?.canGoBack}
        onClick={() => tab && run(api.tabAction(tab.id, "back"))}
      >
        <Icon name="back" />
      </button>
      <button
        className="icon-btn"
        aria-label="Forward"
        title="Forward (Alt+Right)"
        disabled={!tab?.canGoForward}
        onClick={() => tab && run(api.tabAction(tab.id, "forward"))}
      >
        <Icon name="forward" />
      </button>
      {tab?.loading ? (
        <button className="icon-btn" aria-label="Stop" title="Stop loading (Esc)" onClick={() => run(api.tabAction(tab.id, "stop"))}>
          <Icon name="close" />
        </button>
      ) : (
        <button className="icon-btn" aria-label="Reload" title="Reload (Ctrl+R)" disabled={internal} onClick={() => tab && run(api.tabAction(tab.id, "reload"))}>
          <Icon name="reload" />
        </button>
      )}
      <AddressBar tab={tab} />
      {!internal && (
        <button
          className={`icon-btn${bookmark ? " on" : ""}`}
          aria-label={bookmark ? "Edit bookmark" : "Bookmark this page"}
          title={bookmark ? "Edit bookmark (Ctrl+D)" : "Bookmark this page (Ctrl+D)"}
          onClick={() => toggleBookmark()}
        >
          <Icon name="star" filled={!!bookmark} />
        </button>
      )}
      <DownloadButton />
      <button
        ref={menuRef}
        className="icon-btn"
        aria-label="Menu"
        title="Menu"
        onClick={() => {
          const r = menuRef.current!.getBoundingClientRect();
          run(api.showMenu({ kind: "app", x: Math.max(8, r.right - 250), y: r.bottom + 4 }));
        }}
      >
        <Icon name="more" />
      </button>
      {tab?.loading && <div className="loading-bar" role="progressbar" aria-label="Loading" />}
    </div>
  );
}
