import { useEffect, useState } from "react";

// From the shortcut table in app/src-tauri/src/browser/shortcuts.rs (and the README).
const SHORTCUTS: { keys: string[][]; action: string }[] = [
  { keys: [["Ctrl", "K"]], action: "Quick switcher" },
  { keys: [["Ctrl", "T"]], action: "New tab" },
  { keys: [["Ctrl", "W"]], action: "Close tab" },
  { keys: [["Ctrl", "Shift", "T"]], action: "Reopen closed tab" },
  { keys: [["Ctrl", "Tab"], ["Ctrl", "1–9"]], action: "Switch tabs" },
  { keys: [["Ctrl", "L"], ["F6"]], action: "Focus the address bar" },
  { keys: [["Ctrl", "D"]], action: "Bookmark this page" },
  { keys: [["Ctrl", "Shift", "B"]], action: "Show or hide the bookmarks bar" },
  { keys: [["Ctrl", "H"]], action: "History" },
  { keys: [["Ctrl", "J"]], action: "Downloads" },
  { keys: [["Ctrl", "Shift", "N"]], action: "New incognito window" },
  { keys: [["F12"]], action: "Developer tools" },
];

/** Maps a keyboard event to the label used in the table above. */
function label(e: KeyboardEvent) {
  if (e.key === "Control" || e.key === "Meta") return "Ctrl";
  if (e.key === "Shift") return "Shift";
  if (e.key === "Tab") return "Tab";
  if (/^[1-9]$/.test(e.key)) return "1–9";
  if (/^F\d+$/.test(e.key)) return e.key;
  return e.key.length === 1 ? e.key.toUpperCase() : e.key;
}

export function Shortcuts() {
  // Keys held right now, so the table lights up as you press them.
  const [held, setHeld] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.repeat) return;
      const k = label(e);
      setHeld((h) => (h.has(k) ? h : new Set(h).add(k)));
    };
    const up = (e: KeyboardEvent) => {
      const k = label(e);
      setHeld((h) => {
        if (!h.has(k)) return h;
        const next = new Set(h);
        next.delete(k);
        // Letters released after Ctrl never get their own keyup on some layouts; drop them with it.
        return k === "Ctrl" ? new Set() : next;
      });
    };
    const clear = () => setHeld(new Set());
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    window.addEventListener("blur", clear);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
      window.removeEventListener("blur", clear);
    };
  }, []);

  const matches = (combo: string[]) => combo.length === held.size && combo.every((k) => held.has(k));

  return (
    <section className="section shortcuts" id="shortcuts" aria-labelledby="shortcuts-title">
      <div className="container shortcuts__grid">
        <div className="shortcuts__intro">
          <h2 id="shortcuts-title" className="display-3">
            Your hands can stay on the keyboard.
          </h2>
          <p>
            Zuno catches its shortcuts at the WebView2 level, so they work the same whether its own interface or a website has focus. No clicking
            into the toolbar first.
          </p>
          <p className="shortcuts__try">
            <span className="shortcuts__dot" aria-hidden="true" />
            Go on, hold <kbd>Ctrl</kbd> and watch the list.
          </p>
        </div>
        <ul className="shortcuts__list" data-reveal>
          {SHORTCUTS.map((s) => (
            <li key={s.action} className={s.keys.some(matches) ? "is-hit" : undefined}>
              <span className="shortcuts__action">{s.action}</span>
              <span className="shortcuts__keys">
                {s.keys.map((combo, i) => (
                  <span key={i} className="combo">
                    {i > 0 && <span className="combo__or">or</span>}
                    {combo.map((k) => (
                      <kbd key={k} className={`key key--sm${held.has(k) ? " is-down" : ""}`}>
                        {k}
                      </kbd>
                    ))}
                  </span>
                ))}
              </span>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
