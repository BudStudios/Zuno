// The things a browser usually grows, struck through one after another as the section scrolls in.
const LEFT_OUT = ["a sidebar of services", "a news feed", "a shopping assistant", "sponsored tiles", "a crypto wallet", "toolbar creep"];

const FACTS = [
  {
    title: "Every site works",
    body: "Pages are rendered by WebView2, the Chromium engine inside Microsoft Edge, which Windows keeps up to date.",
    tone: "#86a1ad",
  },
  {
    title: "Built in Rust",
    body: "Tabs, storage, downloads and every security check live in a Rust core. The interface on top is a small React app.",
    tone: "#b48a7c",
  },
  {
    title: "Easy to move into",
    body: "Bring bookmarks, history and settings from Chrome, Edge, Brave, Vivaldi, Opera and Firefox in a few clicks.",
    tone: "#8fa58f",
  },
];

export function Intro() {
  return (
    <section className="section intro" id="about" aria-labelledby="intro-title">
      <div className="container">
        <h2 id="intro-title" className="intro__title">
          Most browsers keep adding things. <em>Zuno keeps taking them away.</em>
        </h2>

        <div className="intro__lead">
          <ul className="strikes" aria-label="Things Zuno leaves out" data-reveal>
            {LEFT_OUT.map((t, i) => (
              <li key={t} style={{ "--i": i } as React.CSSProperties}>
                <span>{t}</span>
              </li>
            ))}
          </ul>
          <div className="intro__body">
            <p>
              On screen there is a tab strip, an address bar and, if you want it, a bookmarks bar. History, downloads, bookmarks and settings open
              as ordinary tabs when you ask for them, then get out of the way again.
            </p>
            <p>None of that crossed-out stuff. The page you’re reading gets the whole window.</p>
          </div>
        </div>

        <dl className="facts">
          {FACTS.map((f, i) => (
            <div key={f.title} className="facts__item" data-reveal style={{ "--tone": f.tone, "--d": `${i * 90}ms` } as React.CSSProperties}>
              <dt>{f.title}</dt>
              <dd>{f.body}</dd>
            </div>
          ))}
        </dl>
      </div>
    </section>
  );
}
