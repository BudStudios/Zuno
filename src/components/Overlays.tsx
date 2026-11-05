import { useEffect, useMemo, useRef, useState } from "react";
import { api, errorMessage } from "../lib/api";
import { bookmarksChanged, notify, setState, useStore } from "../lib/store";
import type { Overlay } from "../lib/store";
import { hostOf } from "../lib/format";
import type { AppInfo, AuthorizationRequest, Bookmark, ExternalRequest, QuickResult } from "../lib/types";
import { Favicon, Icon, Logo } from "./Icon";
import { openUrl } from "./BookmarksBar";

const close = () => setState({ overlay: null });

function Scrim({ children, onClose = close }: { children: React.ReactNode; onClose?: () => void }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      {children}
    </div>
  );
}

// ---------------------------------------------------------------------------------------------

const KIND_LABEL = { tab: "Tab", bookmark: "Bookmark", history: "History" } as const;
const SECTION_LABEL = { tab: "Open tabs", bookmark: "Bookmarks", history: "History" } as const;

function QuickSwitcher() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<QuickResult[]>([]);
  const [selected, setSelected] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let alive = true;
    const t = window.setTimeout(() => {
      api
        .quickSearch(query)
        .then((r) => {
          if (!alive) return;
          setResults(r);
          setSelected(0);
        })
        .catch(() => {});
    }, query ? 60 : 0);
    return () => {
      alive = false;
      window.clearTimeout(t);
    };
  }, [query]);

  useEffect(() => {
    listRef.current?.querySelector(".row.selected")?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const open = (r: QuickResult | undefined, newTab: boolean) => {
    if (!r) return;
    close();
    if (r.kind === "tab") api.activate(r.id).catch((e) => notify(errorMessage(e), "error"));
    else openUrl(r.url, newTab);
  };

  return (
    <Scrim>
      <div className="quick" role="dialog" aria-label="Quick switcher">
        <div className="input-row">
          <Icon name="search" />
          <input
            autoFocus
            value={query}
            placeholder="Search tabs, bookmarks and history"
            aria-label="Search tabs, bookmarks and history"
            spellCheck={false}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setSelected((s) => Math.min(s + 1, results.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setSelected((s) => Math.max(s - 1, 0));
              } else if (e.key === "Enter") {
                e.preventDefault();
                open(results[selected], e.ctrlKey || e.altKey);
              }
            }}
          />
          <kbd>Esc</kbd>
        </div>
        <div className="results" ref={listRef} role="listbox">
          {results.length === 0 && <div className="empty">{query ? "No matches" : "No open tabs"}</div>}
          {results.map((r, i) => (
            <div key={`${r.kind}-${r.id}`}>
              {(i === 0 || results[i - 1].kind !== r.kind) && <div className="section">{SECTION_LABEL[r.kind]}</div>}
              <div
                className={`row${i === selected ? " selected" : ""}`}
                role="option"
                aria-selected={i === selected}
                onMouseMove={() => setSelected(i)}
                onClick={(e) => open(r, e.ctrlKey)}
              >
                <Favicon src={r.favicon} />
                <div className="text">
                  <span className="t">{r.title || hostOf(r.url) || r.url}</span>
                  <span className="u">{r.url}</span>
                </div>
                <span className="kind">{KIND_LABEL[r.kind]}</span>
              </div>
            </div>
          ))}
        </div>
        <div className="footer">
          <span>
            <kbd>↑</kbd> <kbd>↓</kbd> to navigate
          </span>
          <span>
            <kbd>Enter</kbd> to open
          </span>
          <span>
            <kbd>Ctrl</kbd>+<kbd>Enter</kbd> in a new tab
          </span>
        </div>
      </div>
    </Scrim>
  );
}

// ---------------------------------------------------------------------------------------------

function BookmarkDialog({ bookmarkId }: { bookmarkId: number }) {
  const [all, setAll] = useState<Bookmark[] | null>(null);
  const [title, setTitle] = useState("");
  const [url, setUrl] = useState("");
  const [parent, setParent] = useState<number>(1);
  const bookmark = all?.find((b) => b.id === bookmarkId);

  useEffect(() => {
    api.bookmarks().then((list) => {
      setAll(list);
      const b = list.find((x) => x.id === bookmarkId);
      if (b) {
        setTitle(b.title);
        setUrl(b.url ?? "");
        setParent(b.parentId ?? 1);
      }
    });
  }, [bookmarkId]);

  const folders = useMemo(() => {
    if (!all) return [];
    const out: { id: number; label: string }[] = [];
    const walk = (parentId: number | null, depth: number) => {
      all
        .filter((b) => b.isFolder && b.parentId === parentId)
        .sort((a, b) => a.position - b.position)
        .forEach((f) => {
          out.push({ id: f.id, label: `${" ".repeat(depth)}${f.title}` });
          walk(f.id, depth + 1);
        });
    };
    walk(null, 0);
    return out;
  }, [all]);

  const save = async () => {
    try {
      await api.bookmarkUpdate(bookmarkId, title, url);
      if (bookmark && parent !== bookmark.parentId) await api.bookmarkMove(bookmarkId, parent, null);
      bookmarksChanged();
      close();
    } catch (e) {
      notify(errorMessage(e), "error");
    }
  };

  const remove = async () => {
    try {
      await api.bookmarkRemove(bookmarkId);
      bookmarksChanged();
      close();
    } catch (e) {
      notify(errorMessage(e), "error");
    }
  };

  return (
    <Scrim>
      <form
        className="dialog"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <h2>Bookmark</h2>
        <p className="sub">Saved to {folders.find((f) => f.id === bookmark?.parentId)?.label.trim() ?? "bookmarks"}.</p>
        <label htmlFor="bm-title">Name</label>
        <input id="bm-title" className="field" value={title} autoFocus onChange={(e) => setTitle(e.target.value)} />
        <label htmlFor="bm-url">Address</label>
        <input id="bm-url" className="field" value={url} spellCheck={false} onChange={(e) => setUrl(e.target.value)} />
        <label htmlFor="bm-folder">Folder</label>
        <select id="bm-folder" className="field" value={parent} onChange={(e) => setParent(Number(e.target.value))}>
          {folders.map((f) => (
            <option key={f.id} value={f.id}>
              {f.label}
            </option>
          ))}
        </select>
        <div className="actions">
          <button type="button" className="btn danger" onClick={remove} style={{ marginRight: "auto" }}>
            Remove
          </button>
          <button type="button" className="btn" onClick={close}>
            Cancel
          </button>
          <button type="submit" className="btn primary">
            Done
          </button>
        </div>
      </form>
    </Scrim>
  );
}

// ---------------------------------------------------------------------------------------------

const REDIRECT_TEXT: Record<AuthorizationRequest["redirectKind"], (t: string | null) => string> = {
  web: (t) => `the website ${t ?? ""}`,
  loopback: (t) => `a desktop application on this computer (${t ?? "localhost"})`,
  app: (t) => `the “${t ?? "unknown"}” application (Zuno will ask before opening it)`,
  none: () => "the provider's default destination",
};

function AuthDialog({ tab, request }: { tab: number; request: AuthorizationRequest }) {
  const provider = request.providerName ?? request.providerHost;
  const go = () => {
    close();
    api.navigateAuthorization(tab, request.url).catch((e) => notify(errorMessage(e), "error"));
  };
  return (
    <Scrim>
      <div className="dialog" role="dialog" aria-labelledby="auth-title" style={{ width: "min(520px, calc(100vw - 32px))" }}>
        <h2 id="auth-title">Sign-in request</h2>
        <p className="sub">
          This address starts an OAuth {request.isOpenid ? "/ OpenID Connect " : ""}authorization with {provider}. You will sign in on{" "}
          {provider}'s own page — Zuno does not approve anything for you.
        </p>
        <dl className="facts">
          <dt>Provider</dt>
          <dd>
            {provider} <span className="muted">({request.providerHost})</span>
          </dd>
          <dt>Application</dt>
          <dd className="mono">{request.clientId}</dd>
          <dt>Requested access</dt>
          <dd>{request.scopes.length ? request.scopes.join(", ") : "Not specified"}</dd>
          <dt>Result goes to</dt>
          <dd>{REDIRECT_TEXT[request.redirectKind](request.redirectTarget)}</dd>
          <dt>Flow</dt>
          <dd className="mono">response_type={request.responseType}</dd>
        </dl>
        <div style={{ marginTop: 14 }}>
          <div className="check-row">
            <span className={request.hasPkce ? "yes" : "no"}>
              <Icon name={request.hasPkce ? "check" : "warning"} size={14} />
            </span>
            {request.hasPkce ? `Protected with PKCE (${request.pkceMethod})` : "No PKCE protection"}
          </div>
          <div className="check-row">
            <span className={request.hasState ? "yes" : "no"}>
              <Icon name={request.hasState ? "check" : "warning"} size={14} />
            </span>
            {request.hasState ? "Includes a state value against request forgery" : "No state value"}
          </div>
        </div>
        {request.warnings.length > 0 && (
          <div className="warn-box">
            {request.warnings.map((w) => (
              <p key={w}>{w}</p>
            ))}
          </div>
        )}
        <div className="actions">
          <button className="btn" onClick={close}>
            Cancel
          </button>
          <button className="btn primary" autoFocus onClick={go}>
            Continue to {provider}
          </button>
        </div>
      </div>
    </Scrim>
  );
}

// ---------------------------------------------------------------------------------------------

function ExternalDialog({ request }: { request: ExternalRequest }) {
  const respond = (allow: boolean) => {
    close();
    api.externalConfirm(request.token, allow).catch((e) => notify(errorMessage(e), "error"));
  };
  return (
    <Scrim onClose={() => respond(false)}>
      <div className="dialog" role="alertdialog" aria-labelledby="ext-title">
        <h2 id="ext-title">Open an application?</h2>
        <p className="sub">
          {request.authCallback
            ? `${request.authCallback} is returning your sign-in to an application on this computer.`
            : "A page wants to open an application on this computer."}
        </p>
        <dl className="facts">
          <dt>Link type</dt>
          <dd className="mono">{request.scheme}:</dd>
          {request.display !== `${request.scheme}:` && (
            <>
              <dt>Address</dt>
              <dd className="mono">{request.display}</dd>
            </>
          )}
        </dl>
        <div className="actions">
          <button className="btn" onClick={() => respond(false)}>
            Cancel
          </button>
          <button className="btn primary" autoFocus onClick={() => respond(true)}>
            Open application
          </button>
        </div>
      </div>
    </Scrim>
  );
}

// ---------------------------------------------------------------------------------------------

function AboutDialog() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => {
    api.appInfo().then(setInfo).catch(() => {});
  }, []);
  return (
    <Scrim>
      <div className="dialog" role="dialog" aria-labelledby="about-title">
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 4 }}>
          <Logo size={26} />
          <h2 id="about-title" style={{ margin: 0 }}>
            Zuno
          </h2>
        </div>
        <p className="sub">Less noise. More web.</p>
        {info && (
          <dl className="facts">
            <dt>Version</dt>
            <dd>{info.version}</dd>
            <dt>Rendering engine</dt>
            <dd>Microsoft Edge WebView2 {info.webviewVersion}</dd>
            <dt>Framework</dt>
            <dd>Tauri {info.tauriVersion}</dd>
            <dt>Platform</dt>
            <dd>{info.platform}</dd>
            <dt>Profile folder</dt>
            <dd className="mono">{info.dataDir}</dd>
          </dl>
        )}
        <div className="actions">
          <button className="btn primary" autoFocus onClick={close}>
            Close
          </button>
        </div>
      </div>
    </Scrim>
  );
}

function ConfirmDialog({ overlay }: { overlay: Extract<Overlay, { kind: "confirm" }> }) {
  return (
    <Scrim>
      <div className="dialog" role="alertdialog" aria-labelledby="confirm-title">
        <h2 id="confirm-title">{overlay.title}</h2>
        <p className="sub">{overlay.message}</p>
        <div className="actions">
          <button className="btn" onClick={close}>
            Cancel
          </button>
          <button
            className={`btn ${overlay.danger ? "danger" : "primary"}`}
            autoFocus
            onClick={() => {
              close();
              overlay.onConfirm();
            }}
          >
            {overlay.confirmLabel}
          </button>
        </div>
      </div>
    </Scrim>
  );
}

export function Overlays() {
  const overlay = useStore((s) => s.overlay);
  if (!overlay) return null;
  switch (overlay.kind) {
    case "quick":
      return <QuickSwitcher />;
    case "bookmark":
      return <BookmarkDialog bookmarkId={overlay.bookmarkId} />;
    case "auth":
      return <AuthDialog tab={overlay.tab} request={overlay.request} />;
    case "external":
      return <ExternalDialog request={overlay.request} />;
    case "about":
      return <AboutDialog />;
    case "confirm":
      return <ConfirmDialog overlay={overlay} />;
  }
}

export function confirmAction(title: string, message: string, confirmLabel: string, onConfirm: () => void, danger = true) {
  setState({ overlay: { kind: "confirm", title, message, confirmLabel, onConfirm, danger } });
}
