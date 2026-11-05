import { Icon } from "./Icon";
import { Shot } from "./Shot";
import { MOD } from "../lib";

// The six tab-group tones, copied from app/src/styles/app.css (dark theme).
const GROUP_TONES = [
  { name: "Slate", color: "#8b93a1" },
  { name: "Sage", color: "#8fa58f" },
  { name: "Sand", color: "#b5a383" },
  { name: "Clay", color: "#b48a7c" },
  { name: "Mist", color: "#86a1ad" },
  { name: "Plum", color: "#a28aa6" },
];

const IMPORT_SOURCES = ["Chrome", "Edge", "Brave", "Vivaldi", "Chromium", "Opera and Opera GX", "Firefox"];

function Feature({
  id,
  tone,
  title,
  children,
  visual,
  flip,
  wide,
}: {
  id: string;
  tone: string;
  title: string;
  children: React.ReactNode;
  visual: React.ReactNode;
  flip?: boolean;
  wide?: boolean;
}) {
  return (
    <article className={`feature${flip ? " feature--flip" : ""}${wide ? " feature--wide" : ""}`} data-reveal id={id} aria-labelledby={`${id}-title`} style={{ "--tone": tone } as React.CSSProperties}>
      <div className="feature__text">
        <h3 id={`${id}-title`} className="feature__title">
          {title}
        </h3>
        {children}
      </div>
      <div className="feature__stage">{visual}</div>
    </article>
  );
}

export function Features() {
  return (
    <section className="section features" id="features" aria-labelledby="features-title">
      <div className="container">
        <header className="section__head">
          <h2 id="features-title" className="display-2">
            A small surface with a lot underneath.
          </h2>
          <p className="section__lede">The features you use every day, each one a keystroke or a click away and invisible the rest of the time.</p>
        </header>

        <Feature
          id="quick-switcher"
          tone="var(--accent)"
          title="Find anything with one shortcut"
          visual={
            <div className="stage stage--quick">
              <Shot name="part-quick" className="stage__ui" alt="The quick switcher with results under Open tabs, Bookmarks and History." />
              <div className="keys" aria-hidden="true">
                <kbd className="key">{MOD}</kbd>
                <kbd className="key">K</kbd>
              </div>
            </div>
          }
        >
          <p>
            Press <kbd>Ctrl</kbd> <kbd>K</kbd> and type. Open tabs, bookmarks and history come back in one list, grouped so you can see where each
            result lives.
          </p>
          <ul className="ticks">
            <li>Arrow keys to move, Enter to open</li>
            <li>Ctrl Enter opens the result in a new tab</li>
            <li>Works from any page, even while a website has focus</li>
          </ul>
        </Feature>

        <Feature
          id="tab-groups"
          tone="#86a1ad"
          wide
          title="Tab groups that stay quiet"
          visual={
            <div className="stage stage--groups">
              <div className="stage__strip">
                <Shot name="part-chrome" className="stage__chrome" alt="Zuno's tab strip with a Research group and a Design group, the address bar and the bookmarks bar." />
              </div>
              <ul className="tones" aria-label="Group colours">
                {GROUP_TONES.map((t) => (
                  <li key={t.name} className="tone-chip" style={{ "--group-color": t.color } as React.CSSProperties}>
                    {t.name}
                  </li>
                ))}
              </ul>
            </div>
          }
        >
          <p>
            Gather related tabs, name the group and give it one of six muted colours. Click its chip to collapse the whole group to a single label,
            and click again to bring it back.
          </p>
          <ul className="ticks">
            <li>Rename with a double-click</li>
            <li>Drag tabs to reorder, middle-click to close</li>
            <li>Groups are restored with your session</li>
          </ul>
        </Feature>

        <Feature
          id="incognito"
          tone="var(--accent)"
          title="Incognito that cleans up after itself"
          visual={
            <div className="stage stage--incognito">
              <Shot name="part-incognito-badge" className="stage__badge" alt="The Incognito badge shown in the tab strip." />
              <Shot
                name="part-incognito"
                className="stage__ui"
                alt="Zuno's incognito page listing what isn't kept after the window closes and what stays visible to others."
              />
            </div>
          }
        >
          <p>
            <kbd>Ctrl</kbd> <kbd>Shift</kbd> <kbd>N</kbd> opens a separate process with private webviews in a throwaway data folder. Zuno deletes the
            folder when you close the window and sweeps it up after a crash.
          </p>
          <ul className="ticks">
            <li>No history, site icons, session or download list written</li>
            <li>The DNS cache is flushed when the window closes</li>
            <li>Optional MAC address randomization while incognito is open</li>
          </ul>
          <p className="feature__note">The incognito page also tells you plainly what stays visible to websites and your network.</p>
        </Feature>

        <Feature
          id="sign-in"
          tone="#8fa58f"
          flip
          title="See what a sign-in link is asking for"
          visual={
            <div className="stage stage--auth stage--ember">
              <Shot
                name="part-auth"
                className="stage__ui"
                alt="A Sign-in request dialog showing provider, application, requested access, where the result goes, and checks for PKCE and state."
              />
            </div>
          }
        >
          <p>
            Paste an OAuth or OpenID Connect authorization link and Zuno stops to show you who is asking, what access they want and where the result
            will be sent, before you continue to the provider’s page.
          </p>
          <ul className="ticks">
            <li>Checks for PKCE and a state value against forgery</li>
            <li>Warns about anything unusual in the request</li>
            <li>App callbacks are handed to Windows only after you confirm</li>
          </ul>
        </Feature>

        <div className="minor" data-reveal>
          <article className="minor__item" style={{ "--tone": "#b5a383" } as React.CSSProperties}>
            <div className="minor__visual">
              <Shot name="part-downloads" alt="The downloads page with one download in progress, one paused and two finished." />
            </div>
            <h3>Downloads you can steer</h3>
            <p>Real progress, pause and resume, cancel and retry. Existing files are never overwritten. A second copy becomes “name (1)”.</p>
          </article>
          <article className="minor__item" style={{ "--tone": "#86a1ad" } as React.CSSProperties}>
            <div className="minor__visual">
              <Shot name="part-privacy" alt="Privacy settings: tracking prevention set to Balanced, with switches for history, passwords and autofill." />
            </div>
            <h3>Tracking prevention, your level</h3>
            <p>Off, Basic, Balanced or Strict, using Microsoft Edge’s tracking prevention lists. Clear history, cookies, cache and more whenever you like.</p>
          </article>
          <article className="minor__item minor__item--text" style={{ "--tone": "#a28aa6" } as React.CSSProperties}>
            <div className="minor__visual minor__visual--list">
              <p className="minor__label">
                <Icon name="import" size={16} /> Import from
              </p>
              <ul>
                {IMPORT_SOURCES.map((s) => (
                  <li key={s}>{s}</li>
                ))}
              </ul>
            </div>
            <h3>Move in without starting over</h3>
            <p>
              Import bookmarks, history and settings, and cookies from Firefox. Zuno shows what each profile has, which settings would change, and a
              summary when it’s done.
            </p>
          </article>
        </div>
      </div>
    </section>
  );
}
