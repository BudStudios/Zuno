// Shapes mirrored from the Rust core (serde camelCase).

export type TabId = number;
export type GroupId = number;

export interface Tab {
  id: TabId;
  url: string;
  title: string;
  favicon: string | null;
  loading: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
  group: GroupId | null;
  error: string | null;
  live: boolean;
}

export interface Group {
  id: GroupId;
  name: string;
  color: string;
  collapsed: boolean;
}

export interface Snapshot {
  tabs: Tab[];
  groups: Group[];
  active: TabId | null;
}

export type Theme = "system" | "light" | "dark";
export type Startup = "newtab" | "restore" | "homepage";
export type TrackingPrevention = "none" | "basic" | "balanced" | "strict";

export interface Settings {
  theme: Theme;
  searchEngine: string;
  customSearchUrl: string;
  startup: Startup;
  homepage: string;
  showBookmarksBar: boolean;
  downloadDir: string | null;
  saveHistory: boolean;
  clearHistoryOnExit: boolean;
  trackingPrevention: TrackingPrevention;
  passwordAutosave: boolean;
  generalAutofill: boolean;
  incognitoRandomizeMac: boolean;
}

export interface SearchEngine {
  id: string;
  name: string;
  template: string;
}

export interface InitialState {
  tabs: Snapshot;
  settings: Settings;
  searchEngines: SearchEngine[];
  groupColors: string[];
  incognito: boolean;
}

export interface Bookmark {
  id: number;
  parentId: number | null;
  isFolder: boolean;
  title: string;
  url: string | null;
  position: number;
  createdAt: number;
}

export interface HistoryEntry {
  id: number;
  url: string;
  title: string;
  visitedAt: number;
}

export type DownloadState = "inprogress" | "paused" | "completed" | "failed" | "cancelled";

export interface DownloadItem {
  id: number;
  url: string;
  path: string;
  fileName: string;
  mimeType: string | null;
  totalBytes: number | null;
  receivedBytes: number;
  state: DownloadState;
  error: string | null;
  canResume: boolean;
  startedAt: number;
  finishedAt: number | null;
  exists: boolean;
}

export type QuickResult =
  | { kind: "tab"; id: TabId; title: string; url: string; favicon: string | null }
  | { kind: "bookmark"; id: number; title: string; url: string; favicon: string | null }
  | { kind: "history"; id: number; title: string; url: string; favicon: string | null; visitedAt: number };

export type RedirectKind = "web" | "loopback" | "app" | "none";

export interface AuthorizationRequest {
  url: string;
  providerHost: string;
  providerName: string | null;
  clientId: string;
  responseType: string;
  redirectUri: string | null;
  redirectKind: RedirectKind;
  redirectTarget: string | null;
  scopes: string[];
  isOpenid: boolean;
  hasPkce: boolean;
  pkceMethod: string | null;
  hasState: boolean;
  warnings: string[];
}

export type NavigateOutcome =
  | { kind: "navigated" }
  | { kind: "external" }
  | { kind: "authReview"; request: AuthorizationRequest };

export type CallbackOutcome =
  | "authorized"
  | "unknown"
  | { denied: { error: string; description: string | null } };

export interface AuthEvent {
  tab: TabId;
  clientId: string;
  provider: string;
  deliveredTo: string;
  outcome: CallbackOutcome;
}

export interface ExternalRequest {
  tab: TabId;
  token: string;
  scheme: string;
  display: string;
  authCallback: string | null;
}

export interface Notice {
  level: "info" | "error";
  message: string;
}

export type UiAction = { action: "renameGroup"; group: GroupId } | { action: "about" };

export type Shortcut =
  | "new-tab"
  | "close-tab"
  | "reopen-closed-tab"
  | "next-tab"
  | "previous-tab"
  | "select-tab1"
  | "select-tab2"
  | "select-tab3"
  | "select-tab4"
  | "select-tab5"
  | "select-tab6"
  | "select-tab7"
  | "select-tab8"
  | "select-last-tab"
  | "focus-address"
  | "quick-switcher"
  | "reload"
  | "back"
  | "forward"
  | "bookmark"
  | "history"
  | "downloads"
  | "bookmarks"
  | "settings"
  | "toggle-bookmarks-bar"
  | "fullscreen";

export type ImportCategory = "bookmarks" | "history" | "cookies" | "passwords" | "settings";

export interface BrowserProfile {
  id: string;
  name: string;
}

export interface BrowserSource {
  id: string;
  name: string;
  engine: "chromium" | "firefox";
  profiles: BrowserProfile[];
}

export interface CategoryInfo {
  category: ImportCategory;
  available: boolean;
  count: number | null;
  reason: string | null;
}

export interface SettingChange {
  key: string;
  label: string;
  current: string;
  imported: string;
}

export interface Inspection {
  categories: CategoryInfo[];
  settingChanges: SettingChange[];
}

export interface CategoryResult {
  category: ImportCategory;
  status: "imported" | "skipped" | "failed" | "unavailable";
  count: number;
  message: string;
}

export interface ImportProgress {
  category: ImportCategory;
  message: string;
  done: number;
  total: number;
}

export interface Account {
  id: string;
  issuer: string;
  clientId: string;
  scopes: string;
  displayName: string | null;
  email: string | null;
  subject: string | null;
  connectedAt: number;
  hasRefreshToken: boolean;
}

export interface AppInfo {
  version: string;
  webviewVersion: string;
  tauriVersion: string;
  dataDir: string;
  platform: string;
}
