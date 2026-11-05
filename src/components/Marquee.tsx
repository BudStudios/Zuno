// A slow ribbon of the things Zuno does, under the hero. Decorative: the same facts are in the
// sections below, so it is hidden from assistive tech.

const ITEMS: { keys?: string[]; text: string; tone: string }[] = [
  { keys: ["Ctrl", "K"], text: "find any tab", tone: "#a59ad0" },
  { text: "tab groups in six quiet colours", tone: "#86a1ad" },
  { keys: ["Ctrl", "Shift", "N"], text: "incognito that deletes itself", tone: "#a28aa6" },
  { text: "imports from Chrome, Edge, Firefox and friends", tone: "#b5a383" },
  { keys: ["Ctrl", "J"], text: "downloads you can pause", tone: "#8fa58f" },
  { text: "tracking prevention, four levels", tone: "#86a1ad" },
  { text: "OAuth links explained before you click", tone: "#b48a7c" },
  { keys: ["Ctrl", "Shift", "T"], text: "bring back that tab", tone: "#a59ad0" },
];

export function Marquee() {
  const row = ITEMS.map((item, i) => (
    <li key={i} className="marquee__item" style={{ "--tone": item.tone } as React.CSSProperties}>
      {item.keys?.map((k) => (
        <kbd key={k} className="key key--sm">
          {k}
        </kbd>
      ))}
      <span>{item.text}</span>
    </li>
  ));
  return (
    <div className="marquee" aria-hidden="true">
      <ul className="marquee__track">
        {row}
        {row}
      </ul>
    </div>
  );
}
