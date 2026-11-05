import { useEffect, useMemo, useState } from "react";
import { api, errorMessage } from "../lib/api";
import { bookmarksChanged, notify, useStore } from "../lib/store";
import { hostOf } from "../lib/format";
import type { Bookmark } from "../lib/types";
import { Favicon, Icon } from "../components/Icon";
import { openUrl } from "../components/BookmarksBar";
import { confirmAction } from "../components/Overlays";

const fail = (e: unknown) => notify(errorMessage(e), "error");

interface Editing {
  id: number | "new" | "new-folder";
  title: string;
  url: string;
  parent: number;
}

export function BookmarksPage() {
  const tick = useStore((s) => s.bookmarksTick);
  const [all, setAll] = useState<Bookmark[]>([]);
  const [folder, setFolder] = useState(1);
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<Editing | null>(null);
  const [icons, setIcons] = useState<Record<string, string>>({});
  const [dragId, setDragId] = useState<number | null>(null);

  useEffect(() => {
    api
      .bookmarks()
      .then((list) => {
        setAll(list);
        const hosts = [...new Set(list.filter((b) => b.url).map((b) => hostOf(b.url!)))];
        api.favicons(hosts).then(setIcons).catch(() => {});
      })
      .catch(fail);
  }, [tick]);

  const folders = useMemo(() => {
    const out: { b: Bookmark; depth: number }[] = [];
    const walk = (parentId: number | null, depth: number) =>
      all
        .filter((b) => b.isFolder && b.parentId === parentId)
        .sort((a, b) => a.position - b.position)
        .forEach((b) => {
          out.push({ b, depth });
          walk(b.id, depth + 1);
        });
    walk(null, 0);
    return out;
  }, [all]);

  const q = query.trim().toLowerCase();
  const items = q
    ? all.filter((b) => !b.isFolder && (b.title.toLowerCase().includes(q) || (b.url ?? "").toLowerCase().includes(q)))
    : all.filter((b) => b.parentId === folder).sort((a, b) => a.position - b.position);
  const current = all.find((b) => b.id === folder);

  const save = async () => {
    if (!editing) return;
    try {
      if (editing.id === "new") await api.bookmarkAdd(editing.parent, editing.title, editing.url);
      else if (editing.id === "new-folder") await api.bookmarkAddFolder(editing.parent, editing.title || "New folder");
      else {
        const original = all.find((b) => b.id === editing.id);
        await api.bookmarkUpdate(editing.id, editing.title, original?.isFolder ? null : editing.url);
        if (original && original.parentId !== editing.parent) await api.bookmarkMove(editing.id, editing.parent, null);
      }
      setEditing(null);
      bookmarksChanged();
    } catch (e) {
      fail(e);
    }
  };

  const remove = (b: Bookmark) => {
    const doIt = () =>
      api
        .bookmarkRemove(b.id)
        .then(bookmarksChanged)
        .catch(fail);
    if (b.isFolder && all.some((x) => x.parentId === b.id)) {
      confirmAction(`Delete “${b.title}”?`, "The folder and everything inside it will be deleted.", "Delete folder", doIt);
    } else doIt();
  };

  const drop = (target: Bookmark | null, parent: number, after: boolean) => {
    if (dragId == null || dragId === target?.id) return;
    let position: number | null = null;
    if (target) {
      const siblings = all.filter((b) => b.parentId === parent && b.id !== dragId).sort((a, b) => a.position - b.position);
      position = siblings.findIndex((s) => s.id === target.id) + (after ? 1 : 0);
    }
    api
      .bookmarkMove(dragId, parent, position)
      .then(bookmarksChanged)
      .catch(fail);
    setDragId(null);
  };

  const editor = (e: Editing, isFolder: boolean) => (
    <form
      className="list-row"
      onSubmit={(ev) => {
        ev.preventDefault();
        save();
      }}
    >
      <div className="inline-edit" style={isFolder ? { gridTemplateColumns: "1fr 160px auto" } : undefined}>
        <input className="field" autoFocus placeholder="Name" value={e.title} onChange={(ev) => setEditing({ ...e, title: ev.target.value })} />
        {!isFolder && (
          <input className="field" placeholder="https://" spellCheck={false} value={e.url} onChange={(ev) => setEditing({ ...e, url: ev.target.value })} />
        )}
        <select className="field" value={e.parent} aria-label="Folder" onChange={(ev) => setEditing({ ...e, parent: Number(ev.target.value) })}>
          {folders
            .filter((f) => f.b.id !== e.id)
            .map((f) => (
              <option key={f.b.id} value={f.b.id}>
                {" ".repeat(f.depth)}
                {f.b.title}
              </option>
            ))}
        </select>
        <div style={{ display: "flex", gap: 6 }}>
          <button type="submit" className="btn primary">
            Save
          </button>
          <button type="button" className="btn ghost" onClick={() => setEditing(null)}>
            Cancel
          </button>
        </div>
      </div>
    </form>
  );

  return (
    <div className="page">
      <div className="page-inner" style={{ maxWidth: 1040 }}>
        <div className="page-head">
          <h1>Bookmarks</h1>
          <span className="spacer" />
          <label className="search-field">
            <Icon name="search" size={14} />
            <input value={query} placeholder="Search bookmarks" aria-label="Search bookmarks" onChange={(e) => setQuery(e.target.value)} />
          </label>
          <button
            className="btn"
            title="Export all bookmarks as an HTML file other browsers can import"
            onClick={() =>
              api
                .bookmarksExport()
                .then((path) => path && notify(`Bookmarks exported to ${path}`))
                .catch(fail)
            }
          >
            Export
          </button>
        </div>
        <div className="bm">
          <div className="tree" role="tree">
            {folders.map(({ b, depth }) => (
              <button
                key={b.id}
                role="treeitem"
                aria-selected={folder === b.id && !q}
                className={folder === b.id && !q ? "on" : ""}
                style={{ paddingLeft: 10 + depth * 16 }}
                onClick={() => {
                  setFolder(b.id);
                  setQuery("");
                  setEditing(null);
                }}
                onDragOver={(e) => dragId != null && e.preventDefault()}
                onDrop={(e) => {
                  e.preventDefault();
                  drop(null, b.id, false);
                }}
              >
                <Icon name="folder" size={14} />
                <span>{b.title}</span>
              </button>
            ))}
          </div>
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 12 }}>
              <div style={{ flex: 1, fontWeight: 600 }}>{q ? `Results for “${query}”` : current?.title}</div>
              {!q && (
                <>
                  <button className="btn" onClick={() => setEditing({ id: "new-folder", title: "", url: "", parent: folder })}>
                    <Icon name="folder" size={14} /> New folder
                  </button>
                  <button className="btn" onClick={() => setEditing({ id: "new", title: "", url: "", parent: folder })}>
                    <Icon name="plus" size={14} /> Add bookmark
                  </button>
                </>
              )}
            </div>
            <div className="list">
              {editing && (editing.id === "new" || editing.id === "new-folder") && editor(editing, editing.id === "new-folder")}
              {items.length === 0 && !editing && (
                <div className="empty-state" style={{ padding: 36 }}>
                  {q ? "No bookmarks match your search" : "This folder is empty"}
                </div>
              )}
              {items.map((b) =>
                editing?.id === b.id ? (
                  <div key={b.id}>{editor(editing, b.isFolder)}</div>
                ) : (
                  <div
                    key={b.id}
                    className="list-row clickable"
                    draggable={!q}
                    onDragStart={() => setDragId(b.id)}
                    onDragEnd={() => setDragId(null)}
                    onDragOver={(e) => dragId != null && e.preventDefault()}
                    onDrop={(e) => {
                      e.preventDefault();
                      const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
                      drop(b, b.parentId ?? folder, e.clientY > rect.top + rect.height / 2);
                    }}
                    onClick={(e) => (b.isFolder ? setFolder(b.id) : openUrl(b.url!, e.ctrlKey))}
                    onAuxClick={(e) => e.button === 1 && b.url && openUrl(b.url, true)}
                  >
                    {b.isFolder ? <Icon name="folder" /> : <Favicon src={icons[hostOf(b.url!)]} />}
                    <div className="main">
                      <div className="t">{b.title}</div>
                      {!b.isFolder && <div className="u">{b.url}</div>}
                    </div>
                    <div className="row-actions">
                      {b.parentId !== null && (
                        <>
                          <button
                            className="icon-btn small"
                            title="Edit"
                            aria-label="Edit"
                            onClick={(e) => {
                              e.stopPropagation();
                              setEditing({ id: b.id, title: b.title, url: b.url ?? "", parent: b.parentId ?? folder });
                            }}
                          >
                            <Icon name="edit" size={14} />
                          </button>
                          <button
                            className="icon-btn small"
                            title="Delete"
                            aria-label="Delete"
                            onClick={(e) => {
                              e.stopPropagation();
                              remove(b);
                            }}
                          >
                            <Icon name="trash" size={14} />
                          </button>
                        </>
                      )}
                    </div>
                  </div>
                ),
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
