// Typed wrappers around the Rust commands. All privileged work happens in Rust; this module only
// marshals arguments.

import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type {
  Account,
  AppInfo,
  Bookmark,
  BrowserSource,
  CategoryResult,
  DownloadItem,
  GroupId,
  HistoryEntry,
  ImportCategory,
  InitialState,
  Inspection,
  NavigateOutcome,
  QuickResult,
  Settings,
  TabId,
  AuthorizationRequest,
} from "./types";

export interface Insets {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export type MenuRequest =
  | { kind: "app"; x: number; y: number }
  | { kind: "tab"; tab: TabId; x: number; y: number }
  | { kind: "group"; group: GroupId; x: number; y: number };

export const api = {
  uiReady: (insets: Insets) => invoke<InitialState>("ui_ready", { insets }),
  setInsets: (insets: Insets) => invoke<void>("layout_set_insets", { insets }),
  setOverlay: (overlay: boolean) => invoke<void>("layout_set_overlay", { overlay }),
  focusContent: () => invoke<void>("focus_content"),
  appInfo: () => invoke<AppInfo>("app_info"),
  openIncognito: () => invoke<void>("incognito_open"),

  newTab: (input?: string) => invoke<TabId>("tab_new", { input: input ?? null }),
  openPage: (page: "settings" | "history" | "bookmarks" | "downloads" | "import" | "newtab") =>
    invoke<TabId>("tab_open_page", { page }),
  closeTab: (id: TabId) => invoke<void>("tab_close", { id }),
  activate: (id: TabId) => invoke<void>("tab_activate", { id }),
  selectIndex: (index: number | null) => invoke<void>("tab_select_index", { index }),
  cycle: (forward: boolean) => invoke<void>("tab_cycle", { forward }),
  moveTab: (id: TabId, index: number) => invoke<void>("tab_move", { id, index }),
  reopenClosed: () => invoke<TabId | null>("tab_reopen_closed"),
  navigate: (id: TabId, input: string) => invoke<NavigateOutcome>("tab_navigate", { id, input }),
  navigateAuthorization: (id: TabId, url: string) => invoke<void>("tab_navigate_authorization", { id, url }),
  tabAction: (id: TabId, action: "back" | "forward" | "reload" | "stop") => invoke<void>("tab_action", { id, action }),
  quickSearch: (query: string) => invoke<QuickResult[]>("quick_search", { query }),
  externalConfirm: (token: string, allow: boolean) => invoke<void>("external_confirm", { token, allow }),

  groupCreate: (name: string, tabs: TabId[]) => invoke<GroupId>("group_create", { name, tabs }),
  groupAssign: (tab: TabId, group: GroupId | null) => invoke<void>("group_assign", { tab, group }),
  groupRename: (group: GroupId, name: string) => invoke<void>("group_rename", { group, name }),
  groupCollapse: (group: GroupId, collapsed: boolean) => invoke<void>("group_collapse", { group, collapsed }),
  groupRemove: (group: GroupId, closeTabs: boolean) => invoke<void>("group_remove", { group, closeTabs }),
  showMenu: (menu: MenuRequest) => invoke<void>("menu_show", { menu }),

  bookmarks: () => invoke<Bookmark[]>("bookmarks_list"),
  bookmarkFind: (url: string) => invoke<Bookmark | null>("bookmark_find", { url }),
  bookmarkAdd: (parent: number | null, title: string, url: string) => invoke<Bookmark>("bookmark_add", { parent, title, url }),
  bookmarkAddFolder: (parent: number | null, title: string) => invoke<Bookmark>("bookmark_add_folder", { parent, title }),
  bookmarkUpdate: (id: number, title: string, url: string | null) => invoke<Bookmark>("bookmark_update", { id, title, url }),
  bookmarkMove: (id: number, parent: number, position: number | null) => invoke<void>("bookmark_move", { id, parent, position }),
  bookmarkRemove: (id: number) => invoke<void>("bookmark_remove", { id }),
  bookmarksExport: () => invoke<string | null>("bookmarks_export"),

  historySearch: (query: string, before: number | null = null, limit = 100) =>
    invoke<HistoryEntry[]>("history_search", { query, before, limit }),
  historyDelete: (id: number) => invoke<void>("history_delete", { id }),
  historyClear: () => invoke<void>("history_clear"),
  topSites: (limit = 8) => invoke<HistoryEntry[]>("top_sites", { limit }),
  favicons: (hosts: string[]) => invoke<Record<string, string>>("favicons", { hosts }),

  downloads: () => invoke<DownloadItem[]>("downloads_list"),
  downloadAction: (id: number, action: "pause" | "resume" | "cancel" | "open" | "show" | "remove") =>
    invoke<void>("download_action", { id, action }),
  downloadsClear: () => invoke<void>("downloads_clear"),
  downloadsOpenFolder: () => invoke<void>("downloads_open_folder"),

  setSettings: (settings: Settings) => invoke<Settings>("settings_set", { settings }),
  chooseDownloadDir: () => invoke<string | null>("choose_download_dir"),
  clearBrowsingData: (request: {
    history: boolean;
    downloads: boolean;
    cookies: boolean;
    cache: boolean;
    siteStorage: boolean;
    passwords: boolean;
    autofill: boolean;
  }) => invoke<void>("clear_browsing_data", { request }),

  importDetect: () => invoke<BrowserSource[]>("import_detect"),
  importInspect: (source: string, profile: string) => invoke<Inspection>("import_inspect", { source, profile }),
  importRun: (request: { source: string; profile: string; categories: ImportCategory[]; confirmSettingsOverwrite: boolean }) =>
    invoke<CategoryResult[]>("import_run", { request }),

  authAnalyze: (url: string) => invoke<AuthorizationRequest | null>("auth_analyze", { url }),
  accounts: () => invoke<Account[]>("accounts_list"),
  accountConnect: (issuer: string, clientId: string, clientSecret: string | null, scopes: string) =>
    invoke<void>("account_connect", { issuer, clientId, clientSecret, scopes }),
  accountDisconnect: (id: string) => invoke<void>("account_disconnect", { id }),
};

/** Subscribes to an event emitted by the Rust core to this interface webview. */
export function on<T>(event: string, handler: (payload: T) => void): () => void {
  let unlisten: (() => void) | null = null;
  let cancelled = false;
  getCurrentWebview()
    .listen<T>(event, (e) => handler(e.payload))
    .then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    })
    .catch((e) => console.error("listen failed", event, e));
  return () => {
    cancelled = true;
    unlisten?.();
  };
}

export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Something went wrong";
}
