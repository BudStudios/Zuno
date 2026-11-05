// Mirrors the architecture described in app/README.md and app/src-tauri/src.

const MODULES = [
  { name: "browser", body: "Tabs, groups, layout and session" },
  { name: "storage", body: "SQLite for bookmarks, history, icons" },
  { name: "downloads", body: "Progress, pause, resume, safe names" },
  { name: "importer", body: "Chromium-family and Firefox" },
  { name: "incognito", body: "Throwaway sessions and cleanup" },
  { name: "authentication", body: "OAuth review, PKCE, accounts" },
  { name: "security", body: "Scheme policy, sanitising, caller checks" },
];

const SPECS = [
  ["Rendering engine", "Microsoft Edge WebView2 (Chromium), updated by Windows"],
  ["Core", "Rust on Tauri 2"],
  ["Interface", "React and TypeScript"],
  ["Storage", "SQLite, in your profile folder"],
  ["Installer", "Per-user, no administrator rights needed"],
  ["Platform", "Windows (x64)"],
];

export function Architecture() {
  return (
    <section className="section arch" id="architecture" aria-labelledby="arch-title">
      <div className="container">
        <header className="section__head">
          <h2 id="arch-title" className="display-2">
            Under the hood.
          </h2>
          <p className="section__lede">
            Every tab is its own WebView2 instance, placed exactly inside the window by the Rust core. Restored tabs load only when you first open
            them.
          </p>
        </header>

        <div className="arch__grid">
          <figure className="diagram" aria-label="Diagram of Zuno's architecture">
            <div className="diagram__window">
              <div className="diagram__label">Main window</div>
              <div className="diagram__ui">
                <strong>Interface webview</strong>
                <span>React: tab strip, toolbar, bookmarks bar, internal pages</span>
              </div>
              <div className="diagram__content">
                <div className="diagram__tabs" aria-hidden="true">
                  <span className="diagram__tab diagram__tab--3" />
                  <span className="diagram__tab diagram__tab--2" />
                </div>
                <div className="diagram__tab diagram__tab--1">
                  <strong>One WebView2 per tab</strong>
                  <span>The active tab is shown, the rest are hidden. No app permissions.</span>
                </div>
              </div>
            </div>
            <div className="diagram__link" aria-hidden="true">
              <span>commands</span>
              <span>events</span>
            </div>
            <div className="diagram__core">
              <div className="diagram__label">Rust core</div>
              <ul>
                {MODULES.map((m) => (
                  <li key={m.name}>
                    <code>{m.name}</code>
                    <span>{m.body}</span>
                  </li>
                ))}
              </ul>
            </div>
            <div className="diagram__engine">
              <strong>Microsoft Edge WebView2 runtime</strong>
              <span>Rendering, tracking prevention, passwords, downloads</span>
            </div>
          </figure>

          <div className="arch__specs">
            <dl className="specs">
              {SPECS.map(([k, v]) => (
                <div key={k}>
                  <dt>{k}</dt>
                  <dd>{v}</dd>
                </div>
              ))}
            </dl>
            <p className="arch__note">
              Menus are native Win32 menus so they draw above website content. Sessions, cookies and site storage live in one persistent profile and
              survive restarts, like any desktop browser.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
