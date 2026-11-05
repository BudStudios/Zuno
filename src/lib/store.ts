// Minimal external store. The Rust core is the source of truth for tabs and settings; this store
// mirrors what it emits plus purely presentational state (overlays, banners).

import { useSyncExternalStore } from "react";
import type {
  AuthEvent,
  AuthorizationRequest,
  DownloadItem,
  ExternalRequest,
  Group,
  SearchEngine,
  Settings,
  Snapshot,
  Tab,
  TabId,
} from "./types";

export type Overlay =
  | { kind: "quick" }
  | { kind: "bookmark"; bookmarkId: number }
  | { kind: "auth"; tab: TabId; request: AuthorizationRequest }
  | { kind: "external"; request: ExternalRequest }
  | { kind: "about" }
  | { kind: "confirm"; title: string; message: string; confirmLabel: string; danger?: boolean; onConfirm: () => void };

export type Banner =
  | { kind: "notice"; id: number; level: "info" | "error"; message: string }
  | { kind: "auth"; id: number; event: AuthEvent };

export interface AppState {
  ready: boolean;
  tabs: Tab[];
  groups: Group[];
  active: TabId | null;
  settings: Settings | null;
  engines: SearchEngine[];
  groupColors: string[];
  downloads: DownloadItem[];
  fullscreen: boolean;
  overlay: Overlay | null;
  banners: Banner[];
  renamingGroup: number | null;
  focusAddressTick: number;
  /** Incremented whenever bookmarks change, so views can refresh. */
  bookmarksTick: number;
  /** This window is incognito: nothing about the browsing is recorded. */
  incognito: boolean;
}

let state: AppState = {
  ready: false,
  tabs: [],
  groups: [],
  active: null,
  settings: null,
  engines: [],
  groupColors: [],
  downloads: [],
  fullscreen: false,
  overlay: null,
  banners: [],
  renamingGroup: null,
  focusAddressTick: 0,
  bookmarksTick: 0,
  incognito: false,
};

const listeners = new Set<() => void>();

export function getState(): AppState {
  return state;
}

export function setState(patch: Partial<AppState> | ((s: AppState) => Partial<AppState>)) {
  const next = typeof patch === "function" ? patch(state) : patch;
  state = { ...state, ...next };
  listeners.forEach((l) => l());
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useStore<T>(selector: (s: AppState) => T): T {
  return useSyncExternalStore(subscribe, () => selector(state));
}

export function applySnapshot(s: Snapshot) {
  setState({ tabs: s.tabs, groups: s.groups, active: s.active });
}

export function applyTab(tab: Tab) {
  setState((s) => ({ tabs: s.tabs.map((t) => (t.id === tab.id ? tab : t)) }));
}

export function applyDownload(item: DownloadItem) {
  setState((s) => {
    const exists = s.downloads.some((d) => d.id === item.id);
    return { downloads: exists ? s.downloads.map((d) => (d.id === item.id ? item : d)) : [item, ...s.downloads] };
  });
}

let bannerSeq = 1;

export function pushBanner(banner: Omit<Banner, "id"> & { kind: Banner["kind"] }, timeoutMs = 6000) {
  const id = bannerSeq++;
  setState((s) => ({ banners: [...s.banners.slice(-2), { ...banner, id } as Banner] }));
  if (timeoutMs > 0) window.setTimeout(() => dismissBanner(id), timeoutMs);
}

export function notify(message: string, level: "info" | "error" = "info") {
  pushBanner({ kind: "notice", level, message } as Omit<Banner, "id">, level === "error" ? 8000 : 4000);
}

export function dismissBanner(id: number) {
  setState((s) => ({ banners: s.banners.filter((b) => b.id !== id) }));
}

export function activeTab(s: AppState = state): Tab | undefined {
  return s.tabs.find((t) => t.id === s.active);
}

export function bookmarksChanged() {
  setState((s) => ({ bookmarksTick: s.bookmarksTick + 1 }));
}
