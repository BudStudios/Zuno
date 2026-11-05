import { useCallback, useEffect, useState } from "react";
import { Nav } from "./components/Nav";
import { Hero } from "./components/Hero";
import { Marquee } from "./components/Marquee";
import { Intro } from "./components/Intro";
import { Compare } from "./components/Compare";
import { Features } from "./components/Features";
import { Shortcuts } from "./components/Shortcuts";
import { Tour } from "./components/Tour";
import { Privacy } from "./components/Privacy";
import { Architecture } from "./components/Architecture";
import { Roadmap } from "./components/Roadmap";
import { GetZuno } from "./components/GetZuno";
import { Footer } from "./components/Footer";
import { Palette } from "./components/Palette";
import { useReveal } from "./lib";

export function App() {
  const [palette, setPalette] = useState(false);
  const [notice, setNotice] = useState<{ id: number; message: string } | null>(null);

  const openPalette = useCallback(() => setPalette(true), []);
  const closePalette = useCallback(() => setPalette(false), []);
  const showNotice = useCallback((message: string) => setNotice({ id: Date.now(), message }), []);

  useReveal();

  // Ctrl+K / Cmd+K toggles the page quick switcher, as it does in Zuno.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPalette((p) => !p);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("no-scroll", palette);
  }, [palette]);

  useEffect(() => {
    if (!notice) return;
    const t = window.setTimeout(() => setNotice(null), 3200);
    return () => window.clearTimeout(t);
  }, [notice]);

  return (
    <>
      <a className="skip" href="#about">
        Skip to content
      </a>
      <Nav onSearch={openPalette} />
      <main>
        <Hero onSearch={openPalette} />
        <Marquee />
        <Intro />
        <Compare />
        <Features />
        <Shortcuts />
        <Tour />
        <Privacy />
        <Architecture />
        <Roadmap />
        <GetZuno />
      </main>
      <Footer />
      <Palette open={palette} onClose={closePalette} onNotice={showNotice} />
      <div className="toast-region" role="status" aria-live="polite">
        {notice && (
          <div key={notice.id} className="toast">
            {notice.message}
          </div>
        )}
      </div>
    </>
  );
}
