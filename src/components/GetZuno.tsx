import { Icon, Logo } from "./Icon";
import { INSTALLER, formatBytes } from "../lib";

// The installer is whichever Zuno_x.y.z_x64-setup.exe sits in download/ (see tools/installer.mjs).
// Install behaviour comes from app/src-tauri/tauri.conf.json: per-user NSIS, WebView2 bootstrapper.
const NEXT = [
  { title: "Run the installer", body: "Double-click the file once it lands in your Downloads folder." },
  { title: "No admin prompt", body: "Zuno installs for your account only, so you don’t need administrator rights." },
  { title: "Start browsing", body: "If your PC doesn’t have the WebView2 runtime yet, the installer fetches it for you." },
];

export function GetZuno() {
  return (
    <section className="section get" id="get" aria-labelledby="get-title">
      <div className="container">
        <h2 id="get-title" className="display-1 get__title">
          Give the web the <span>whole window.</span>
        </h2>

        <div className="dl" data-reveal>
          <div className="dl__main">
            <div className="dl__app">
              <span className="dl__icon">
                <Logo size={30} />
              </span>
              <div>
                <p className="dl__name">Zuno for Windows</p>
                <p className="dl__sub">{INSTALLER ? `Version ${INSTALLER.version} · 64-bit` : "64-bit"}</p>
              </div>
            </div>

            {INSTALLER ? (
              <>
                <a className="btn btn--primary btn--xl dl__btn" href={INSTALLER.url} download={INSTALLER.file}>
                  <Icon name="download" size={18} />
                  Download Zuno
                  <span className="dl__size">{formatBytes(INSTALLER.bytes)}</span>
                </a>
                <p className="dl__file">
                  <Icon name="file" size={14} />
                  <code>{INSTALLER.file}</code>
                </p>
              </>
            ) : (
              <p className="dl__missing">The installer isn’t up yet. Check back soon.</p>
            )}

            <ul className="dl__facts">
              <li>
                <Icon name="windows" size={14} /> Windows 10 and 11
              </li>
              <li>
                <Icon name="check" size={14} /> Free
              </li>
              <li>
                <Icon name="lock" size={14} /> No account needed
              </li>
            </ul>
          </div>

          <ol className="dl__next">
            {NEXT.map((n, i) => (
              <li key={n.title}>
                <span className="dl__n" aria-hidden="true">
                  {i + 1}
                </span>
                <div>
                  <h3>{n.title}</h3>
                  <p>{n.body}</p>
                </div>
              </li>
            ))}
          </ol>
        </div>
      </div>
    </section>
  );
}
