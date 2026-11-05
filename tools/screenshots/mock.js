// Injected before Zuno's interface loads: stands in for the Tauri IPC bridge with demo data so the
// real React UI (app/dist) renders outside the desktop shell. Receives a scenario object.
(scenario) => {
  const now = Date.now();
  const H = 3600_000;
  const icon = (letter, bg, fg = "#f5f5f5") =>
    "data:image/svg+xml," +
    encodeURIComponent(
      `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" rx="8" fill="${bg}"/><text x="16" y="22" font-family="Segoe UI,Arial" font-weight="700" font-size="17" text-anchor="middle" fill="${fg}">${letter}</text></svg>`,
    );
  const fav = {
    "www.rust-lang.org": icon("R", "#3b3530", "#e0c9a6"),
    "doc.rust-lang.org": icon("R", "#3b3530", "#e0c9a6"),
    "v2.tauri.app": icon("T", "#1f3a40", "#8fd3dd"),
    "learn.microsoft.com": icon("M", "#23324a", "#9cc2f0"),
    "developer.mozilla.org": icon("M", "#2b2b2b", "#f5f5f5"),
    "news.ycombinator.com": icon("Y", "#5a3b25", "#f3c79b"),
    "en.wikipedia.org": icon("W", "#e9e9e6", "#222"),
    "github.com": icon("G", "#2a2c31", "#f5f5f5"),
    "www.figma.com": icon("F", "#3a2c40", "#d9b8e6"),
    "fonts.google.com": icon("A", "#2b3a2e", "#b8dcbc"),
    "duckduckgo.com": icon("D", "#4a2e26", "#f2b9a3"),
    "www.theverge.com": icon("V", "#3d2a45", "#e5b6f2"),
    "archive.org": icon("A", "#2e2e2e", "#d8d8d8"),
  };
  const host = (u) => {
    try {
      return new URL(u).host;
    } catch {
      return "";
    }
  };
  const tab = (id, url, title, group = null, extra = {}) => ({
    id,
    url,
    title,
    favicon: fav[host(url)] ?? null,
    loading: false,
    canGoBack: false,
    canGoForward: false,
    group,
    error: null,
    live: true,
    ...extra,
  });

  const incognito = !!scenario.incognito;
  const tabs = incognito
    ? [tab(1, scenario.url ?? "zuno://newtab", "New Tab")]
    : [
        tab(1, "https://www.rust-lang.org/", "Rust Programming Language", 1),
        tab(2, "https://v2.tauri.app/concept/architecture/", "Tauri Architecture | Tauri", 1),
        tab(3, "https://learn.microsoft.com/microsoft-edge/webview2/", "Introduction to Microsoft Edge WebView2", 1),
        tab(4, "https://www.figma.com/files/recents", "Recents – Figma", 2),
        tab(5, "https://fonts.google.com/", "Browse Fonts – Google Fonts", 2),
        tab(6, "https://news.ycombinator.com/", "Hacker News"),
        tab(7, scenario.url ?? "zuno://newtab", scenario.title ?? "New Tab"),
      ];
  const groups = incognito
    ? []
    : [
        { id: 1, name: "Research", color: "mist", collapsed: !!scenario.collapseResearch },
        { id: 2, name: "Design", color: "plum", collapsed: false },
      ];
  const active = tabs[tabs.length - 1].id;

  const settings = {
    theme: scenario.theme ?? "dark",
    searchEngine: "duckduckgo",
    customSearchUrl: "",
    startup: "restore",
    homepage: "",
    showBookmarksBar: !incognito,
    downloadDir: null,
    saveHistory: true,
    clearHistoryOnExit: false,
    trackingPrevention: "balanced",
    passwordAutosave: true,
    generalAutofill: true,
    incognitoRandomizeMac: false,
  };
  const engines = [
    { id: "duckduckgo", name: "DuckDuckGo", template: "" },
    { id: "google", name: "Google", template: "" },
    { id: "bing", name: "Bing", template: "" },
    { id: "brave", name: "Brave Search", template: "" },
    { id: "startpage", name: "Startpage", template: "" },
    { id: "ecosia", name: "Ecosia", template: "" },
  ];

  const bm = (id, parentId, title, url, position) => ({ id, parentId, isFolder: !url, title, url: url ?? null, position, createdAt: now - 30 * 24 * H });
  const bookmarks = [
    bm(1, null, "Bookmarks bar", null, 0),
    bm(2, null, "Other bookmarks", null, 1),
    bm(10, 1, "Dev", null, 0),
    bm(11, 1, "Rust Book", "https://doc.rust-lang.org/book/", 1),
    bm(12, 1, "MDN", "https://developer.mozilla.org/", 2),
    bm(13, 1, "GitHub", "https://github.com/", 3),
    bm(14, 1, "Wikipedia", "https://en.wikipedia.org/", 4),
    bm(15, 1, "Hacker News", "https://news.ycombinator.com/", 5),
    bm(20, 10, "Tauri Docs", "https://v2.tauri.app/", 0),
    bm(21, 10, "WebView2 API reference", "https://learn.microsoft.com/microsoft-edge/webview2/reference/win32/", 1),
  ];

  const hist = (id, url, title, hoursAgo) => ({ id, url, title, visitedAt: now - hoursAgo * H });
  const history = [
    hist(1, "https://www.rust-lang.org/", "Rust Programming Language", 0.2),
    hist(2, "https://v2.tauri.app/concept/architecture/", "Tauri Architecture | Tauri", 0.5),
    hist(3, "https://learn.microsoft.com/microsoft-edge/webview2/", "Introduction to Microsoft Edge WebView2", 0.9),
    hist(4, "https://www.figma.com/files/recents", "Recents – Figma", 1.4),
    hist(5, "https://fonts.google.com/", "Browse Fonts – Google Fonts", 1.8),
    hist(6, "https://news.ycombinator.com/", "Hacker News", 2.3),
    hist(7, "https://doc.rust-lang.org/book/ch16-00-concurrency.html", "Fearless Concurrency – The Rust Programming Language", 26),
    hist(8, "https://en.wikipedia.org/wiki/Typography", "Typography – Wikipedia", 27.5),
    hist(9, "https://developer.mozilla.org/en-US/docs/Web/CSS/color-mix", "color-mix() – CSS | MDN", 28),
    hist(10, "https://archive.org/", "Internet Archive", 29),
    hist(11, "https://www.theverge.com/tech", "Tech – The Verge", 30),
  ];
  const topSites = [
    hist(1, "https://www.rust-lang.org/", "Rust", 0),
    hist(2, "https://github.com/", "GitHub", 0),
    hist(3, "https://developer.mozilla.org/", "MDN Web Docs", 0),
    hist(4, "https://news.ycombinator.com/", "Hacker News", 0),
    hist(5, "https://en.wikipedia.org/", "Wikipedia", 0),
    hist(6, "https://www.figma.com/", "Figma", 0),
    hist(7, "https://fonts.google.com/", "Google Fonts", 0),
    hist(8, "https://archive.org/", "Internet Archive", 0),
  ];

  const MB = 1024 * 1024;
  const downloads = [
    { id: 3, url: "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe", path: "C:\\Users\\you\\Downloads\\rustup-init.exe", fileName: "rustup-init.exe", mimeType: "application/octet-stream", totalBytes: 13.4 * MB, receivedBytes: 8.9 * MB, state: "inprogress", error: null, canResume: false, startedAt: now - 0.05 * H, finishedAt: null, exists: true },
    { id: 2, url: "https://archive.org/download/sample/field-recordings.zip", path: "C:\\Users\\you\\Downloads\\field-recordings.zip", fileName: "field-recordings.zip", mimeType: "application/zip", totalBytes: 412 * MB, receivedBytes: 131 * MB, state: "paused", error: null, canResume: true, startedAt: now - 0.4 * H, finishedAt: null, exists: true },
    { id: 1, url: "https://fonts.google.com/download/inter.zip", path: "C:\\Users\\you\\Downloads\\Inter.zip", fileName: "Inter.zip", mimeType: "application/zip", totalBytes: 5.6 * MB, receivedBytes: 5.6 * MB, state: "completed", error: null, canResume: false, startedAt: now - 1.2 * H, finishedAt: now - 1.19 * H, exists: true },
    { id: 0, url: "https://example.com/brand-guidelines.pdf", path: "C:\\Users\\you\\Downloads\\brand-guidelines (1).pdf", fileName: "brand-guidelines (1).pdf", mimeType: "application/pdf", totalBytes: 2.1 * MB, receivedBytes: 2.1 * MB, state: "completed", error: null, canResume: false, startedAt: now - 1.5 * H, finishedAt: now - 1.49 * H, exists: true },
  ];

  const authRequest = {
    url: "https://accounts.example.com/oauth2/authorize?client_id=notes-desktop&response_type=code",
    providerHost: "accounts.example.com",
    providerName: "Example Accounts",
    clientId: "notes-desktop.apps.example.com",
    responseType: "code",
    redirectUri: "http://127.0.0.1:53682/callback",
    redirectKind: "loopback",
    redirectTarget: "127.0.0.1:53682",
    scopes: ["openid", "email", "profile"],
    isOpenid: true,
    hasPkce: true,
    pkceMethod: "S256",
    hasState: true,
    warnings: [],
  };

  const sources = [
    { id: "chrome", name: "Google Chrome", engine: "chromium", profiles: [{ id: "Default", name: "Person 1" }] },
    { id: "edge", name: "Microsoft Edge", engine: "chromium", profiles: [{ id: "Default", name: "Profile 1" }] },
    { id: "firefox", name: "Mozilla Firefox", engine: "firefox", profiles: [{ id: "default-release", name: "default-release" }] },
  ];

  const quick = (q) => {
    q = (q || "").toLowerCase();
    const match = (x) => !q || x.title.toLowerCase().includes(q) || x.url.toLowerCase().includes(q);
    const out = tabs.filter((t) => !t.url.startsWith("zuno://") && match(t)).map((t) => ({ kind: "tab", id: t.id, title: t.title, url: t.url, favicon: t.favicon }));
    if (!q) return out;
    bookmarks.filter((b) => b.url && match(b)).forEach((b) => out.push({ kind: "bookmark", id: b.id, title: b.title, url: b.url, favicon: fav[host(b.url)] ?? null }));
    history.filter(match).slice(0, 3).forEach((h) => out.push({ kind: "history", id: h.id, title: h.title, url: h.url, favicon: fav[host(h.url)] ?? null, visitedAt: h.visitedAt }));
    return out;
  };

  const handlers = {
    ui_ready: () => ({ tabs: { tabs, groups, active }, settings, searchEngines: engines, groupColors: ["slate", "sage", "sand", "clay", "mist", "plum"], incognito }),
    downloads_list: () => downloads,
    bookmarks_list: () => bookmarks,
    bookmark_find: () => null,
    favicons: ({ hosts }) => Object.fromEntries(hosts.filter((h) => fav[h]).map((h) => [h, fav[h]])),
    top_sites: () => topSites,
    history_search: ({ before }) => (before ? [] : history),
    quick_search: ({ query }) => quick(query),
    tab_navigate: () => (scenario.authOnNavigate ? { kind: "authReview", request: authRequest } : { kind: "navigated" }),
    settings_set: ({ settings: s }) => s,
    accounts_list: () => [],
    app_info: () => ({ version: "0.1.0", webviewVersion: "142.0.3595.53", tauriVersion: "2.12.5", dataDir: "C:\\Users\\you\\AppData\\Roaming\\app.zuno.browser", platform: "Windows 11 (x64)" }),
    import_detect: () => sources,
    "plugin:event|listen": () => Math.floor(Math.random() * 1e9),
  };

  const callbacks = {};
  const listeners = {};
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "ui" } },
    transformCallback(cb) {
      const id = Math.floor(Math.random() * 1e9);
      callbacks[id] = cb;
      return id;
    },
    unregisterCallback(id) {
      delete callbacks[id];
    },
    convertFileSrc: (p) => p,
    async invoke(cmd, args = {}) {
      if (cmd === "plugin:event|listen") {
        (listeners[args.event] ??= []).push(args.handler);
      }
      const h = handlers[cmd];
      return h ? h(args) : null;
    },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  window.__zunoEmit = (event, payload) => (listeners[event] ?? []).forEach((id) => callbacks[id]?.({ event, id: 0, payload }));
};
