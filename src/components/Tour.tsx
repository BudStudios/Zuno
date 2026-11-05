// An interactive tour of Zuno's pages. The selector is styled after Zuno's own tab strip and the
// theme control after its segmented control; every image is a capture of the real interface.

import { useEffect, useRef, useState } from "react";
import { Icon, type IconName } from "./Icon";
import { Shot, shotUrl, type ShotName } from "./Shot";

const VIEWS: { id: ShotName; label: string; icon: IconName; title: string; body: string }[] = [
  {
    id: "newtab",
    label: "New tab",
    icon: "tab",
    title: "A new tab page with nothing to scroll",
    body: "A search box and the sites you visit most. Search goes to DuckDuckGo, Google, Bing, Brave Search, Startpage, Ecosia or an address you set yourself.",
  },
  {
    id: "quick-switcher",
    label: "Quick switcher",
    icon: "search",
    title: "Tabs, bookmarks and history in one search",
    body: "Ctrl K opens it over any page. Results are grouped by where they come from, and the keyboard does everything.",
  },
  {
    id: "settings-privacy",
    label: "Settings",
    icon: "settings",
    title: "Settings you can read in one sitting",
    body: "Appearance, search engine, privacy, downloads, startup, accounts, import and export. Every option says what it does in plain words.",
  },
  {
    id: "history",
    label: "History",
    icon: "history",
    title: "History grouped by day",
    body: "Search it, delete single visits or clear it all. Zuno can clear it when it closes, and sign-in codes and tokens in addresses are never recorded.",
  },
  {
    id: "bookmarks",
    label: "Bookmarks",
    icon: "layers",
    title: "Bookmarks with real folders",
    body: "Edit, move between folders, drag to reorder and search. Export to an HTML file that any browser can import.",
  },
  {
    id: "downloads",
    label: "Downloads",
    icon: "download",
    title: "Downloads you can pause",
    body: "Progress, pause and resume, cancel, open the file or show it in its folder. Zuno never overwrites a file that's already there.",
  },
  {
    id: "import",
    label: "Import",
    icon: "import",
    title: "Import from the browser you're leaving",
    body: "Pick a browser and a profile. Zuno shows what each category holds before it copies anything, then a summary of what came across.",
  },
  {
    id: "incognito",
    label: "Incognito",
    icon: "incognito",
    title: "Incognito, explained on the page",
    body: "What isn't kept and what is still visible to others, right where you start browsing. The window is marked with a lavender badge.",
  },
];

export function Tour() {
  const [index, setIndex] = useState(0);
  const [light, setLight] = useState(false);
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const sectionRef = useRef<HTMLElement>(null);
  const view = VIEWS[index];

  // Warm the cache once the tour is close to the viewport so switching views is instant.
  useEffect(() => {
    const el = sectionRef.current;
    if (!el) return;
    const io = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        io.disconnect();
        const retina = window.devicePixelRatio > 1;
        for (const v of VIEWS) {
          new Image().src = shotUrl(v.id, retina);
        }
      },
      { rootMargin: "400px" },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  useEffect(() => {
    if (!light) return;
    const retina = window.devicePixelRatio > 1;
    for (const v of VIEWS) new Image().src = shotUrl(`${v.id}-light`, retina);
  }, [light]);

  const select = (i: number, focus = false) => {
    const next = (i + VIEWS.length) % VIEWS.length;
    setIndex(next);
    if (focus) {
      tabRefs.current[next]?.focus();
      tabRefs.current[next]?.scrollIntoView({ block: "nearest", inline: "nearest" });
    }
  };

  return (
    <section className="section tour" id="tour" ref={sectionRef} aria-labelledby="tour-title">
      <div className="container">
        <header className="section__head section__head--row">
          <div>
            <h2 id="tour-title" className="display-2">
              A look around.
            </h2>
            <p className="section__lede">Every page of Zuno, in both of its themes.</p>
          </div>
          <div className="segmented" role="radiogroup" aria-label="Theme">
            <button role="radio" aria-checked={!light} className={!light ? "on" : ""} onClick={() => setLight(false)}>
              <Icon name="moon" size={14} /> Dark
            </button>
            <button role="radio" aria-checked={light} className={light ? "on" : ""} onClick={() => setLight(true)}>
              <Icon name="sun" size={14} /> Light
            </button>
          </div>
        </header>

        <div className="tour__layout">
          <aside className="tour__side">
            <div className="tour__nav" role="tablist" aria-label="Zuno pages" aria-orientation="vertical">
              {VIEWS.map((v, i) => (
                <button
                  key={v.id}
                  ref={(el) => {
                    tabRefs.current[i] = el;
                  }}
                  role="tab"
                  id={`tour-tab-${v.id}`}
                  aria-selected={i === index}
                  aria-controls="tour-panel"
                  tabIndex={i === index ? 0 : -1}
                  className={`tour__tab${i === index ? " tour__tab--active" : ""}`}
                  onClick={() => select(i)}
                  onKeyDown={(e) => {
                    if (e.key === "ArrowDown" || e.key === "ArrowRight") select(i + 1, true);
                    else if (e.key === "ArrowUp" || e.key === "ArrowLeft") select(i - 1, true);
                    else if (e.key === "Home") select(0, true);
                    else if (e.key === "End") select(VIEWS.length - 1, true);
                    else return;
                    e.preventDefault();
                  }}
                >
                  <Icon name={v.icon} size={15} />
                  <span>{v.label}</span>
                </button>
              ))}
            </div>
            <div className="tour__caption" aria-live="polite">
              <h3>{view.title}</h3>
              <p>{view.body}</p>
            </div>
          </aside>

          <div className={`tour__frame${light ? " tour__frame--light" : ""}`}>
            <div className="tour__viewport" id="tour-panel" role="tabpanel" aria-labelledby={`tour-tab-${view.id}`}>
              <Shot
                key={`${view.id}-${light}`}
                name={view.id}
                light={light}
                className="tour__shot"
                alt={`Zuno ${view.label} page, ${light ? "light" : "dark"} theme. ${view.title}.`}
              />
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
