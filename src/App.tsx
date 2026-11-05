import { useEffect, useLayoutEffect, useRef } from "react";
import { api, errorMessage, on } from "./lib/api";
import {
  activeTab,
  applyDownload,
  applySnapshot,
  applyTab,
  getState,
  notify,
  pushBanner,
  setState,
  useStore,
} from "./lib/store";
import type { AuthEvent, DownloadItem, ExternalRequest, Notice, Settings, Shortcut, Snapshot, Tab, UiAction } from "./lib/types";
import { internalPage } from "./lib/format";
import { TabStrip } from "./components/TabStrip";
import { Toolbar, toggleBookmark } from "./components/Toolbar";
import { BookmarksBar } from "./components/BookmarksBar";
import { Banners } from "./components/Banners";
import { Overlays } from "./components/Overlays";
import { NewTabPage } from "./pages/NewTab";
import { SettingsPage } from "./pages/Settings";
import { HistoryPage } from "./pages/History";
import { BookmarksPage } from "./pages/Bookmarks";
import { DownloadsPage } from "./pages/Downloads";
import { ImportPage } from "./pages/Import";

function resolveTheme(theme: Settings["theme"] | undefined): "dark" | "light" {
  if (theme === "light" || theme === "dark") return theme;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function handleShortcut(s: Shortcut) {
  const st = getState();
  const tab = activeTab(st);
  const run = (p: Promise<unknown>) => p.catch((e) => notify(errorMessage(e), "error"));
  const selectTab = s.match(/^select-tab(\d)$/);
  if (selectTab) return run(api.selectIndex(Number(selectTab[1]) - 1));
  switch (s) {
    case "new-tab":
      return run(api.newTab());
    case "close-tab":
      return tab && run(api.closeTab(tab.id));
    case "reopen-closed-tab":
      return run(api.reopenClosed());
    case "next-tab":
      return run(api.cycle(true));
    case "previous-tab":
      return run(api.cycle(false));
    case "select-last-tab":
      return run(api.selectIndex(null));
    case "focus-address":
      return setState((x) => ({ focusAddressTick: x.focusAddressTick + 1 }));
    case "quick-switcher":
      return setState((x) => ({ overlay: x.overlay?.kind === "quick" ? null : { kind: "quick" } }));
    case "reload":
      return tab && run(api.tabAction(tab.id, "reload"));
    case "back":
      return tab?.canGoBack && run(api.tabAction(tab.id, "back"));
    case "forward":
      return tab?.canGoForward && run(api.tabAction(tab.id, "forward"));
    case "bookmark":
      return toggleBookmark();
    case "history":
      return run(api.openPage("history"));
    case "downloads":
      return run(api.openPage("downloads"));
    case "bookmarks":
      return run(api.openPage("bookmarks"));
    case "settings":
      return run(api.openPage("settings"));
    case "toggle-bookmarks-bar":
      if (st.settings) {
        return run(api.setSettings({ ...st.settings, showBookmarksBar: !st.settings.showBookmarksBar }).then((settings) => setState({ settings })));
      }
      return;
    case "fullscreen":
      return;
  }
}

function InternalView({ url }: { url: string }) {
  switch (internalPage(url)) {
    case "settings":
      return <SettingsPage />;
    case "history":
      return <HistoryPage />;
    case "bookmarks":
      return <BookmarksPage />;
    case "downloads":
      return <DownloadsPage />;
    case "import":
      return <ImportPage />;
    default:
      return <NewTabPage />;
  }
}

export function App() {
  const chromeRef = useRef<HTMLDivElement>(null);
  const ready = useStore((s) => s.ready);
  const theme = useStore((s) => s.settings?.theme);
  const overlay = useStore((s) => s.overlay);
  const fullscreen = useStore((s) => s.fullscreen);
  const showBar = useStore((s) => s.settings?.showBookmarksBar ?? false);
  const tab = useStore((s) => activeTab(s));
  const url = tab?.url ?? "";
  const internal = url.startsWith("zuno://");

  // Theme: applied instantly; "system" follows Windows.
  useLayoutEffect(() => {
    const apply = () => (document.documentElement.dataset.theme = resolveTheme(theme));
    apply();
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [theme]);

  // Event subscriptions from the Rust core.
  useEffect(() => {
    const subs = [
      on<Snapshot>("zuno://tabs", applySnapshot),
      on<Tab>("zuno://tab", applyTab),
      on<DownloadItem>("zuno://download", applyDownload),
      on<Shortcut>("zuno://shortcut", handleShortcut),
      on<Settings>("zuno://settings", (settings) => setState({ settings })),
      on<boolean>("zuno://fullscreen", (fullscreen) => setState({ fullscreen })),
      on<Notice>("zuno://notice", (n) => notify(n.message, n.level)),
      on<AuthEvent>("zuno://auth", (event) => pushBanner({ kind: "auth", event } as never, 12000)),
      on<ExternalRequest>("zuno://external", (request) => setState({ overlay: { kind: "external", request } })),
      on<{ ok: boolean; message: string }>("zuno://account", (e) => notify(e.message, e.ok ? "info" : "error")),
      on<UiAction>("zuno://ui-action", (a) => {
        if (a.action === "renameGroup") setState({ renamingGroup: a.group });
        if (a.action === "about") setState({ overlay: { kind: "about" } });
      }),
    ];
    return () => subs.forEach((u) => u());
  }, []);

  // Report the interface's size so website views are placed exactly inside the content area.
  useLayoutEffect(() => {
    const el = chromeRef.current;
    if (!el) return;
    const insets = () => ({ top: Math.round(el.getBoundingClientRect().height), right: 0, bottom: 0, left: 0 });
    let lastTop = -1;
    api
      .uiReady(insets())
      .then((init) => {
        applySnapshot(init.tabs);
        setState({ ready: true, settings: init.settings, engines: init.searchEngines, groupColors: init.groupColors, incognito: init.incognito });
        if (init.incognito) document.documentElement.dataset.incognito = "";
        lastTop = insets().top;
      })
      .catch((e) => notify(errorMessage(e), "error"));
    api.downloads().then((downloads) => setState({ downloads })).catch(() => {});
    const ro = new ResizeObserver(() => {
      const next = insets();
      if (next.top !== lastTop && lastTop !== -1) {
        lastTop = next.top;
        api.setInsets(next).catch(() => {});
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Website views are native windows above this document; hide them while a modal is open.
  const hasOverlay = overlay !== null;
  useEffect(() => {
    if (!ready) return;
    api.setOverlay(hasOverlay).catch(() => {});
    if (!hasOverlay && !internal) api.focusContent().catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hasOverlay, ready]);

  return (
    <div className={`app${fullscreen ? " fullscreen-content" : ""}`}>
      <div className="chrome" ref={chromeRef}>
        <TabStrip />
        <Toolbar />
        {showBar && <BookmarksBar />}
        <Banners />
      </div>
      <div className="content">{ready && tab && internal && <InternalView key={tab.id + url} url={url} />}</div>
      <Overlays />
    </div>
  );
}
