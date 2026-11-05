import { Logo } from "./Icon";
import { INSTALLER, scrollToId } from "../lib";

const COLUMNS: { title: string; links: { id: string; label: string }[] }[] = [
  {
    title: "Product",
    links: [
      { id: "features", label: "Features" },
      { id: "compare", label: "Zuno vs Chrome" },
      { id: "tour", label: "Tour" },
      { id: "shortcuts", label: "Keyboard shortcuts" },
      { id: "roadmap", label: "Roadmap" },
      { id: "get", label: "Download" },
    ],
  },
  {
    title: "Details",
    links: [
      { id: "privacy", label: "Privacy and security" },
      { id: "architecture", label: "Under the hood" },
      { id: "incognito", label: "Incognito" },
      { id: "sign-in", label: "Sign-in review" },
    ],
  },
];

export function Footer() {
  const go = (id: string) => (e: React.MouseEvent) => {
    e.preventDefault();
    scrollToId(id);
  };
  return (
    <footer className="footer">
      <div className="container footer__grid">
        <div className="footer__brand">
          <a href="#top" onClick={go("top")} className="footer__logo" aria-label="Zuno, back to top">
            <Logo size={22} className="nav__mark" />
            Zuno
          </a>
          <p>Less noise. More web.</p>
          <p className="footer__meta">{INSTALLER ? `Version ${INSTALLER.version} for Windows` : "For Windows"}</p>
        </div>
        {COLUMNS.map((c) => (
          <nav key={c.title} className="footer__col" aria-label={c.title}>
            <h2>{c.title}</h2>
            <ul>
              {c.links.map((l) => (
                <li key={l.id}>
                  <a href={`#${l.id}`} onClick={go(l.id)}>
                    {l.label}
                  </a>
                </li>
              ))}
            </ul>
          </nav>
        ))}
      </div>
      <div className="container footer__bottom">
        <span>Copyright © 2026 Zuno</span>
        <span>Websites in Zuno are rendered by Microsoft Edge WebView2. Microsoft Edge and WebView2 are trademarks of Microsoft.</span>
      </div>
    </footer>
  );
}
