import { INSTALLER } from "../lib";

// Where Zuno goes from here. Plans, not promises: edit the stages as the work firms up.
const STAGES = [
  {
    when: INSTALLER ? `Now · ${INSTALLER.version}` : "Now",
    title: "A quiet browser that works",
    status: "Shipped",
    items: [
      "Tabs, tab groups, the quick switcher, incognito and sign-in review",
      "Import from Chromium-family browsers and Firefox",
      "About 600–700 MB under a heavy load, most of it WebView2",
    ],
  },
  {
    when: "Next",
    title: "Lighter",
    status: "In progress",
    items: [
      "Background tabs go to sleep and give their memory back until you return",
      "A smaller footprint for Zuno’s own interface and Rust core",
      "Tuned WebView2 settings to bring the total well below today’s figure",
    ],
  },
  {
    when: "Later",
    title: "More convenient",
    status: "Planned",
    items: [
      "Separate profiles, so work and personal browsing stay apart",
      "Updates that download and install themselves",
      "A quick switcher that runs commands and settings, not just finds pages",
    ],
  },
];

export function Roadmap() {
  return (
    <section className="section roadmap" id="roadmap" aria-labelledby="roadmap-title">
      <div className="container">
        <header className="section__head">
          <h2 id="roadmap-title" className="display-2">
            Where Zuno is going.
          </h2>
          <p className="section__lede">
            Launch was the first step. From here the work is about making Zuno lighter on your PC and easier to live in, without adding noise
            back.
          </p>
        </header>

        <ol className="roadmap__track">
          {STAGES.map((s, i) => (
            <li key={s.when} className={`roadmap__stage${i === 0 ? " is-now" : ""}`} data-reveal style={{ "--d": `${i * 110}ms` } as React.CSSProperties}>
              <span className="roadmap__dot" aria-hidden="true" />
              <p className="roadmap__when">
                {s.when}
                <span className="roadmap__status">{s.status}</span>
              </p>
              <h3>{s.title}</h3>
              <ul>
                {s.items.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>
            </li>
          ))}
        </ol>

        <p className="roadmap__note">Plans, not promises. Each stage ships when it’s ready, and nothing on it adds a sidebar.</p>
      </div>
    </section>
  );
}
