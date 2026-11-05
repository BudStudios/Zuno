import { Fragment, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, errorMessage } from "../lib/api";
import { notify, setState, useStore } from "../lib/store";
import type { Group, Tab } from "../lib/types";
import { Favicon, Icon, Logo } from "./Icon";

const run = (p: Promise<unknown>) => p.catch((e) => notify(errorMessage(e), "error"));

function GroupChip({ group, count }: { group: Group; count: number }) {
  const renaming = useStore((s) => s.renamingGroup === group.id);
  const [name, setName] = useState(group.name);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (renaming) {
      setName(group.name);
      requestAnimationFrame(() => inputRef.current?.select());
    }
  }, [renaming, group.name]);

  const commit = () => {
    setState({ renamingGroup: null });
    if (name.trim() && name.trim() !== group.name) run(api.groupRename(group.id, name.trim()));
  };

  return (
    <div
      className="group-chip"
      style={{ "--group-color": `var(--g-${group.color})` } as React.CSSProperties}
      title={group.collapsed ? `${group.name} — collapsed (${count} tabs)` : group.name}
      onClick={() => !renaming && run(api.groupCollapse(group.id, !group.collapsed))}
      onDoubleClick={() => setState({ renamingGroup: group.id })}
      onContextMenu={(e) => {
        e.preventDefault();
        run(api.showMenu({ kind: "group", group: group.id, x: e.clientX, y: e.clientY }));
      }}
    >
      {renaming ? (
        <input
          ref={inputRef}
          value={name}
          maxLength={40}
          aria-label="Group name"
          onChange={(e) => setName(e.target.value)}
          onClick={(e) => e.stopPropagation()}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
            if (e.key === "Escape") setState({ renamingGroup: null });
          }}
        />
      ) : (
        <>
          <span className="name">{group.name}</span>
          {group.collapsed && <span className="count">{count}</span>}
        </>
      )}
    </div>
  );
}

function TabItem({
  tab,
  active,
  narrow,
  group,
  dragState,
  onDrag,
}: {
  tab: Tab;
  active: boolean;
  narrow: boolean;
  group: Group | undefined;
  dragState: "before" | "after" | "dragging" | null;
  onDrag: (e: React.DragEvent, tab: Tab, phase: "start" | "over" | "drop" | "end") => void;
}) {
  return (
    <div
      className={`tab${active ? " active" : ""}${narrow ? " narrow" : ""}${dragState === "dragging" ? " dragging" : ""}${
        dragState === "before" ? " drop-before" : dragState === "after" ? " drop-after" : ""
      }`}
      data-group={group ? group.id : undefined}
      style={group ? ({ "--group-color": `var(--g-${group.color})` } as React.CSSProperties) : undefined}
      title={tab.error ? `${tab.title}\n${tab.error}` : tab.title}
      role="tab"
      aria-selected={active}
      draggable
      onMouseDown={(e) => {
        if (e.button === 0 && !active) run(api.activate(tab.id));
      }}
      onAuxClick={(e) => {
        if (e.button === 1) run(api.closeTab(tab.id));
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        run(api.showMenu({ kind: "tab", tab: tab.id, x: e.clientX, y: e.clientY }));
      }}
      onDragStart={(e) => onDrag(e, tab, "start")}
      onDragOver={(e) => onDrag(e, tab, "over")}
      onDrop={(e) => onDrag(e, tab, "drop")}
      onDragEnd={(e) => onDrag(e, tab, "end")}
    >
      {tab.url.startsWith("zuno://") ? <Logo size={16} /> : <Favicon src={tab.favicon} loading={tab.loading} />}
      <span className="title">{tab.title || "Untitled"}</span>
      {tab.error && !tab.loading && <span className="error-dot" aria-label="Page error" />}
      <button
        className="icon-btn small close"
        aria-label="Close tab"
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          run(api.closeTab(tab.id));
        }}
      >
        <Icon name="close" size={14} />
      </button>
    </div>
  );
}

function useMaximized() {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    const w = getCurrentWindow();
    w.isMaximized().then(setMaximized).catch(() => {});
    const unlisten = w.onResized(() => w.isMaximized().then(setMaximized).catch(() => {}));
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);
  return maximized;
}

/**
 * The top window edge lies inside the client area, which the interface webview covers, so
 * resizing from it is started here. The other edges use the native invisible resize frame.
 */
function TopResizeEdge() {
  const w = getCurrentWindow();
  const start = (dir: "North" | "NorthWest" | "NorthEast") => (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    w.startResizeDragging(dir).catch(() => {});
  };
  return (
    <>
      <div className="resize-edge top" onMouseDown={start("North")} />
      <div className="resize-edge top-left" onMouseDown={start("NorthWest")} />
      <div className="resize-edge top-right" onMouseDown={start("NorthEast")} />
    </>
  );
}

function WindowControls({ maximized }: { maximized: boolean }) {
  const w = getCurrentWindow();
  return (
    <div className="window-controls">
      <button aria-label="Minimize" onClick={() => w.minimize()}>
        <Icon name="minimize" />
      </button>
      <button aria-label={maximized ? "Restore" : "Maximize"} onClick={() => w.toggleMaximize()}>
        <Icon name={maximized ? "restore" : "maximize"} size={14} />
      </button>
      <button className="close" aria-label="Close" onClick={() => w.close()}>
        <Icon name="close" />
      </button>
    </div>
  );
}

export function TabStrip() {
  const tabs = useStore((s) => s.tabs);
  const groups = useStore((s) => s.groups);
  const active = useStore((s) => s.active);
  const incognito = useStore((s) => s.incognito);
  const stripRef = useRef<HTMLDivElement>(null);
  const maximized = useMaximized();
  const [width, setWidth] = useState(800);
  const [drag, setDrag] = useState<{ id: number; over: number | null; after: boolean } | null>(null);

  useEffect(() => {
    const el = stripRef.current?.parentElement;
    if (!el) return;
    const ro = new ResizeObserver(() => setWidth(el.getBoundingClientRect().width));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const groupById = new Map(groups.map((g) => [g.id, g]));
  const visible = tabs.filter((t) => !(t.group && groupById.get(t.group)?.collapsed));
  // Space left for tabs after window controls, new-tab button, group chips and a drag gap.
  const available = width - 138 - 40 - 64 - groups.length * 90 - (incognito ? 110 : 0);
  const narrow = visible.length > 0 && available / visible.length < 84;

  const onDrag = (e: React.DragEvent, tab: Tab, phase: "start" | "over" | "drop" | "end") => {
    if (phase === "start") {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/x-zuno-tab", String(tab.id));
      setDrag({ id: tab.id, over: null, after: false });
    } else if (phase === "over" && drag) {
      e.preventDefault();
      const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
      const after = e.clientX > rect.left + rect.width / 2;
      if (drag.over !== tab.id || drag.after !== after) setDrag({ ...drag, over: tab.id, after });
    } else if (phase === "drop" && drag) {
      e.preventDefault();
      const rest = tabs.filter((t) => t.id !== drag.id);
      let index = rest.findIndex((t) => t.id === tab.id);
      if (index >= 0 && drag.id !== tab.id) {
        if (drag.after) index += 1;
        run(api.moveTab(drag.id, index));
      }
      setDrag(null);
    } else if (phase === "end") {
      setDrag(null);
    }
  };

  let previousGroup: number | null = null;
  return (
    <div className="tabbar" data-tauri-drag-region>
      <div className="tabs" ref={stripRef} role="tablist" data-tauri-drag-region>
        {tabs.map((tab) => {
          const group = tab.group ? groupById.get(tab.group) : undefined;
          const chip =
            group && tab.group !== previousGroup ? (
              <GroupChip group={group} count={tabs.filter((t) => t.group === group.id).length} />
            ) : null;
          previousGroup = tab.group;
          if (group?.collapsed) return <Fragment key={tab.id}>{chip}</Fragment>;
          const dragState =
            drag?.id === tab.id ? "dragging" : drag?.over === tab.id ? (drag.after ? "after" : "before") : null;
          return (
            <Fragment key={tab.id}>
              {chip}
              <TabItem tab={tab} active={tab.id === active} narrow={narrow} group={group} dragState={dragState} onDrag={onDrag} />
            </Fragment>
          );
        })}
      </div>
      <button className="icon-btn new-tab-btn" aria-label="New tab" title="New tab (Ctrl+T)" onClick={() => run(api.newTab())}>
        <Icon name="plus" />
      </button>
      <div className="drag-space" data-tauri-drag-region />
      {incognito && (
        <div className="incognito-badge" title="Incognito: history, cookies and site data are not kept after this window closes" data-tauri-drag-region>
          <Icon name="incognito" />
          Incognito
        </div>
      )}
      <WindowControls maximized={maximized} />
      {!maximized && <TopResizeEdge />}
    </div>
  );
}
