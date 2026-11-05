import { useEffect, useState } from "react";
import { GitHubMark, Icon, Logo } from "./Icon";
import { MOD, SECTIONS, scrollToId } from "../lib";

export function Nav({ onSearch }: { onSearch: () => void }) {
  const [scrolled, setScrolled] = useState(false);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 8);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    const onResize = () => window.innerWidth > 1080 && setOpen(false);
    window.addEventListener("keydown", onKey);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", onResize);
    };
  }, [open]);

  const go = (id: string) => (e: React.MouseEvent) => {
    e.preventDefault();
    setOpen(false);
    scrollToId(id);
  };

  return (
    <header className={`nav${scrolled || open ? " nav--solid" : ""}${open ? " nav--open" : ""}`}>
      <div className="nav__inner">
        <a className="nav__brand" href="#top" onClick={go("top")} aria-label="Zuno, back to top">
          <Logo size={22} className="nav__mark" />
          Zuno
        </a>
        <nav className="nav__links" aria-label="Sections">
          {SECTIONS.filter((s) => s.nav).map((s) => (
            <a key={s.id} href={`#${s.id}`} onClick={go(s.id)}>
              {s.title}
            </a>
          ))}
        </nav>
        <div className="nav__actions">
          <button className="nav__search" onClick={onSearch} aria-label="Search this page" title="Search this page">
            <Icon name="search" size={14} />
            <span className="nav__search-label">Search</span>
            <span className="nav__search-keys" aria-hidden="true">
              <kbd>{MOD}</kbd>
              <kbd>K</kbd>
            </span>
          </button>
          <a className="nav__github" href="https://github.com/BudStudios/Zuno" target="_blank" rel="noopener noreferrer" aria-label="Zuno on GitHub" title="Zuno on GitHub">
            <GitHubMark size={16} />
          </a>
          <a className="btn btn--primary nav__cta" href="#get" onClick={go("get")}>
            <Icon name="download" size={14} />
            Download
          </a>
          <button className="nav__menu" aria-label={open ? "Close menu" : "Open menu"} aria-expanded={open} aria-controls="mobile-menu" onClick={() => setOpen((o) => !o)}>
            <Icon name={open ? "close" : "menu"} size={18} />
          </button>
        </div>
      </div>
      <div id="mobile-menu" className="nav__sheet" hidden={!open}>
        {SECTIONS.filter((s) => s.nav || s.id === "get").map((s) => (
          <a key={s.id} href={`#${s.id}`} onClick={go(s.id)}>
            <span>{s.title}</span>
            <span className="nav__sheet-hint">{s.hint}</span>
          </a>
        ))}
      </div>
    </header>
  );
}
