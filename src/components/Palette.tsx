// The page's own quick switcher, built from the same styles as Zuno's (app/src/styles/app.css
// ".quick"): search the page's sections, arrow keys to move, Enter to go.

import { useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "./Icon";
import { INSTALLER, SECTIONS, formatBytes, scrollToId, startDownload } from "../lib";

type Item = { kind: "section"; id: string; title: string; hint: string } | { kind: "action"; id: string; title: string; hint: string; run: () => void };

export function Palette({ open, onClose, onNotice }: { open: boolean; onClose: () => void; onNotice: (msg: string) => void }) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const returnFocus = useRef<HTMLElement | null>(null);

  const items = useMemo<Item[]>(() => {
    const all: Item[] = [
      ...SECTIONS.map((s) => ({ kind: "section" as const, id: s.id, title: s.title, hint: s.hint })),
    ];
    if (INSTALLER) {
      const { file, bytes, version } = INSTALLER;
      all.push({
        kind: "action",
        id: "download",
        title: "Download Zuno",
        hint: `${file}, ${formatBytes(bytes)}`,
        run: () => startDownload() && onNotice(`Downloading Zuno ${version}`),
      });
    }
    const q = query.trim().toLowerCase();
    if (!q) return all;
    return all.filter((i) => i.title.toLowerCase().includes(q) || i.hint.toLowerCase().includes(q));
  }, [query, onNotice]);

  useEffect(() => {
    if (open) {
      returnFocus.current = document.activeElement as HTMLElement | null;
      setQuery("");
      setSelected(0);
    } else {
      returnFocus.current?.focus?.();
    }
  }, [open]);

  useEffect(() => setSelected(0), [query]);

  useEffect(() => {
    listRef.current?.querySelector(".quick__row--selected")?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  if (!open) return null;

  const choose = (item: Item | undefined) => {
    if (!item) return;
    onClose();
    if (item.kind === "action") item.run();
    else requestAnimationFrame(() => scrollToId(item.id));
  };

  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="quick" role="dialog" aria-modal="true" aria-label="Search this page">
        <div className="quick__input">
          <Icon name="search" />
          <input
            autoFocus
            value={query}
            placeholder="Search this page"
            aria-label="Search this page"
            aria-controls="quick-results"
            aria-activedescendant={items[selected] ? `quick-${items[selected].id}` : undefined}
            spellCheck={false}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setSelected((s) => Math.min(s + 1, items.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setSelected((s) => Math.max(s - 1, 0));
              } else if (e.key === "Enter") {
                e.preventDefault();
                choose(items[selected]);
              } else if (e.key === "Escape") {
                e.preventDefault();
                onClose();
              } else if (e.key === "Tab") {
                e.preventDefault();
              }
            }}
          />
          <kbd>Esc</kbd>
        </div>
        <div className="quick__results" id="quick-results" ref={listRef} role="listbox" aria-label="Results">
          {items.length === 0 && <div className="quick__empty">No matches</div>}
          {items.map((item, i) => (
            <div key={item.id}>
              {(i === 0 || items[i - 1].kind !== item.kind) && <div className="quick__section">{item.kind === "section" ? "On this page" : "Actions"}</div>}
              <div
                id={`quick-${item.id}`}
                className={`quick__row${i === selected ? " quick__row--selected" : ""}`}
                role="option"
                aria-selected={i === selected}
                onMouseMove={() => setSelected(i)}
                onClick={() => choose(item)}
              >
                <span className="quick__icon">
                  <Icon name={item.kind === "action" ? "download" : "link"} size={14} />
                </span>
                <span className="quick__text">
                  <span className="quick__t">{item.title}</span>
                  <span className="quick__u">{item.hint}</span>
                </span>
                <span className="quick__kind">{item.kind === "section" ? "Section" : "Action"}</span>
              </div>
            </div>
          ))}
        </div>
        <div className="quick__footer">
          <span>
            <kbd>↑</kbd> <kbd>↓</kbd> to navigate
          </span>
          <span>
            <kbd>Enter</kbd> to open
          </span>
          <span>
            <kbd>Esc</kbd> to close
          </span>
        </div>
      </div>
    </div>
  );
}
