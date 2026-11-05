// Page structure shared by the navigation, the footer and the page quick switcher.

import { useEffect } from "react";

export interface PageSection {
  id: string;
  title: string;
  hint: string;
  nav?: boolean;
}

export const SECTIONS: PageSection[] = [
  { id: "top", title: "Home", hint: "Back to the top" },
  { id: "about", title: "What Zuno is", hint: "A browser that keeps out of the way" },
  { id: "compare", title: "Vs Chrome", hint: "Same engine, about half the memory", nav: true },
  { id: "features", title: "Features", hint: "Quick switcher, tab groups, incognito, sign-in review", nav: true },
  { id: "quick-switcher", title: "Quick switcher", hint: "Ctrl K for tabs, bookmarks and history" },
  { id: "tab-groups", title: "Tab groups", hint: "Six muted colours, collapse to a chip" },
  { id: "incognito", title: "Incognito", hint: "A separate process with a throwaway folder" },
  { id: "sign-in", title: "Sign-in review", hint: "See what an OAuth link asks for" },
  { id: "shortcuts", title: "Keyboard shortcuts", hint: "They work even when a website has focus" },
  { id: "tour", title: "Tour", hint: "Every page of Zuno, in light and dark", nav: true },
  { id: "privacy", title: "Privacy and security", hint: "What Zuno protects, and what it doesn't claim to", nav: true },
  { id: "architecture", title: "Under the hood", hint: "Rust core, React interface, WebView2", nav: true },
  { id: "roadmap", title: "Roadmap", hint: "From launch to lighter and more convenient", nav: true },
  { id: "get", title: "Download", hint: "The Windows installer" },
];

/** The installer in public/download/, with its URL resolved against the page's base. */
export const INSTALLER = __INSTALLER__ && { ...__INSTALLER__, url: `${import.meta.env.BASE_URL}${__INSTALLER__.href}` };

export function formatBytes(bytes: number) {
  return bytes >= 1024 * 1024 ? `${(bytes / (1024 * 1024)).toFixed(1)} MB` : `${Math.round(bytes / 1024)} KB`;
}

/** Starts the installer download from script (used by the page quick switcher). */
export function startDownload() {
  if (!INSTALLER) return false;
  const a = document.createElement("a");
  a.href = INSTALLER.url;
  a.download = INSTALLER.file;
  document.body.appendChild(a);
  a.click();
  a.remove();
  return true;
}

export const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
export const MOD = isMac ? "⌘" : "Ctrl";

export function scrollToId(id: string) {
  const el = document.getElementById(id);
  if (!el) return;
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  el.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "start" });
  history.replaceState(null, "", id === "top" ? location.pathname : `#${id}`);
}

/** Adds .is-in to every [data-reveal] element the first time it scrolls into view. */
export function useReveal() {
  useEffect(() => {
    const els = document.querySelectorAll<HTMLElement>("[data-reveal]");
    if (!("IntersectionObserver" in window)) {
      els.forEach((el) => el.classList.add("is-in"));
      return;
    }
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (!e.isIntersecting) continue;
          e.target.classList.add("is-in");
          io.unobserve(e.target);
        }
      },
      { rootMargin: "0px 0px -12% 0px" },
    );
    els.forEach((el) => io.observe(el));
    return () => io.disconnect();
  }, []);
}
