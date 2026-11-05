import { useEffect, useState } from "react";
import { api, errorMessage } from "../lib/api";
import { activeTab, getState, notify, useStore } from "../lib/store";
import { hostOf, isInternal } from "../lib/format";
import type { Bookmark } from "../lib/types";
import { Favicon, Icon } from "./Icon";

const BOOKMARKS_BAR = 1;

export function openUrl(url: string, newTab: boolean) {
  const tab = activeTab(getState());
  const p = newTab || !tab ? api.newTab(url) : api.navigate(tab.id, url);
  p.catch((e) => notify(errorMessage(e), "error"));
}

export function BookmarksBar() {
  const tick = useStore((s) => s.bookmarksTick);
  const [items, setItems] = useState<Bookmark[]>([]);
  const [icons, setIcons] = useState<Record<string, string>>({});

  useEffect(() => {
    api
      .bookmarks()
      .then((all) => {
        const bar = all.filter((b) => b.parentId === BOOKMARKS_BAR).sort((a, b) => a.position - b.position);
        setItems(bar);
        const hosts = bar.filter((b) => b.url).map((b) => hostOf(b.url!));
        api.favicons(hosts).then(setIcons).catch(() => {});
      })
      .catch(() => {});
  }, [tick]);

  return (
    <div className="bookmarks-bar">
      {items.length === 0 && <span className="empty">Bookmarks you add to the bookmarks bar appear here</span>}
      {items.map((b) =>
        b.isFolder ? (
          <button key={b.id} className="chip" title={b.title} onClick={() => api.openPage("bookmarks")}>
            <Icon name="folder" size={14} />
            <span>{b.title}</span>
          </button>
        ) : (
          <button
            key={b.id}
            className="chip"
            title={`${b.title}\n${b.url}`}
            onClick={(e) => openUrl(b.url!, e.ctrlKey || e.metaKey)}
            onAuxClick={(e) => e.button === 1 && openUrl(b.url!, true)}
          >
            <Favicon src={icons[hostOf(b.url!)]} size={14} />
            <span>{b.title || (isInternal(b.url!) ? b.url : hostOf(b.url!))}</span>
          </button>
        ),
      )}
    </div>
  );
}
