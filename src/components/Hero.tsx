import { Icon } from "./Icon";
import { Shot } from "./Shot";
import { INSTALLER, MOD, formatBytes, scrollToId } from "../lib";

export function Hero({ onSearch }: { onSearch: () => void }) {
  const go = (id: string) => (e: React.MouseEvent) => {
    e.preventDefault();
    scrollToId(id);
  };

  return (
    <section className="hero" id="top" aria-labelledby="hero-title">
      <div className="hero__copy">
        {INSTALLER && (
          <a className="hero__pill" href="#get" onClick={go("get")}>
            <span className="hero__pill-tag">New</span>
            Zuno {INSTALLER.version} is out
            <Icon name="arrow" size={14} />
          </a>
        )}
        <h1 id="hero-title" className="hero__title">
          <span>Less noise.</span>{" "}
          <span className="hero__more">More web.</span>
        </h1>
        <p className="hero__lede">
          A browser for Windows that gets out of your way. Tabs up top, an address bar, and then just the page — rendered by the same engine as
          Microsoft Edge.
        </p>
        <div className="hero__ctas">
          {INSTALLER ? (
            <a className="btn btn--primary btn--lg" href={INSTALLER.url} download={INSTALLER.file}>
              <Icon name="download" size={16} />
              Download for Windows
            </a>
          ) : (
            <a className="btn btn--primary btn--lg" href="#get" onClick={go("get")}>
              Get Zuno for Windows
            </a>
          )}
          <a className="btn btn--ghost btn--lg" href="#tour" onClick={go("tour")}>
            Look around first
          </a>
        </div>
        <p className="hero__meta">
          {INSTALLER ? `Free · ${formatBytes(INSTALLER.bytes)} · Windows 10 and 11, 64-bit` : "Free · Windows 10 and 11, 64-bit"}
          <button className="hero__hint" onClick={onSearch}>
            or press <kbd>{MOD}</kbd> <kbd>K</kbd>
          </button>
        </p>
      </div>

      <div className="hero__stage">
        <figure className="hero__window">
          <Shot name="zuno-newtab" eager alt="Zuno's new tab page: the Zuno mark, the tagline “Less noise. More web.” and a search box, under a single tab and the address bar." />
        </figure>
        <figure className="hero__float">
          <Shot name="part-quick" eager alt="Zuno's quick switcher searching for “rust”, with results grouped into open tabs, bookmarks and history." />
        </figure>
        <span className="hero__note" aria-hidden="true">
          that's the whole UI
          <svg viewBox="0 0 48 48" width="40" height="40">
            <path d="M4 8c20-2 34 10 36 32M40 40l-8-7M40 40l4-10" />
          </svg>
        </span>
      </div>
    </section>
  );
}
