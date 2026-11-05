import { scrollToId } from "../lib";

// Memory figures are the Zuno team's own readings of total memory use under a heavy browsing load.

const SCALE_GB = 1.6;

const METERS = [
  { name: "Zuno", lo: 0.6, hi: 0.7, label: "600–700 MB", tone: "var(--accent)" },
  { name: "Google Chrome", lo: 1.0, hi: 1.5, label: "1–1.5 GB", tone: "#6d6f77" },
];

const TICKS = [0, 0.5, 1, 1.5];

const ROWS: [string, string, string][] = [
  ["Memory under heavy load", "About 600–700 MB", "About 1–1.5 GB"],
  ["Rendering engine", "Chromium, through Microsoft Edge WebView2", "Chromium"],
  ["Engine updates", "Delivered by Windows", "Chrome’s own updater"],
  ["On screen", "Tabs, address bar, optional bookmarks bar", "Tabs, toolbar, side panel and Google services"],
  ["Account", "Nothing to sign in to", "Google account encouraged for sync"],
  ["Platforms", "Windows 10 and 11", "Windows, macOS, Linux, Android, iOS"],
];

const pct = (gb: number) => `${(gb / SCALE_GB) * 100}%`;

export function Compare() {
  return (
    <section className="section compare" id="compare" aria-labelledby="compare-title">
      <div className="container">
        <header className="section__head">
          <h2 id="compare-title" className="display-2">
            Same engine as Chrome. <span className="compare__accent">About half the memory.</span>
          </h2>
          <p className="section__lede">
            Zuno and Chrome both render pages with Chromium, so sites look and behave the same. What changes is everything Zuno doesn’t carry
            around with it.
          </p>
        </header>

        <div className="compare__grid">
          <figure className="meter" data-reveal>
            <figcaption className="meter__title">Memory in use under a heavy load</figcaption>
            <div className="meter__rows">
              {METERS.map((m) => (
                <div key={m.name} className="meter__row" style={{ "--tone": m.tone } as React.CSSProperties}>
                  <div className="meter__label">
                    <strong>{m.name}</strong>
                    <span>{m.label}</span>
                  </div>
                  <div className="meter__track" role="img" aria-label={`${m.name}: ${m.label}`}>
                    <span className="meter__solid" style={{ width: pct(m.lo) }} />
                    <span className="meter__range" style={{ left: pct(m.lo), width: pct(m.hi - m.lo) }} />
                  </div>
                </div>
              ))}
            </div>
            <div className="meter__axis" aria-hidden="true">
              {TICKS.map((t) => (
                <span key={t} style={{ left: pct(t) }}>
                  {t === 0 ? "0" : `${t} GB`}
                </span>
              ))}
            </div>
            <p className="meter__note">
              Most of Zuno’s figure is the WebView2 runtime itself, not Zuno. Bringing it down further is the first thing on the{" "}
              <a
                href="#roadmap"
                onClick={(e) => {
                  e.preventDefault();
                  scrollToId("roadmap");
                }}
              >
                roadmap
              </a>. Your numbers will depend on the sites you open and how many tabs you keep.
            </p>
          </figure>

          <table className="versus">
            <caption className="sr-only">Zuno compared with Google Chrome</caption>
            <thead>
              <tr>
                <td />
                <th scope="col">Zuno</th>
                <th scope="col">Chrome</th>
              </tr>
            </thead>
            <tbody>
              {ROWS.map(([k, zuno, chrome]) => (
                <tr key={k}>
                  <th scope="row">{k}</th>
                  <td>{zuno}</td>
                  <td>{chrome}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}
