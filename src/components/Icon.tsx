// A single consistent icon set: 16px grid, 1.5px strokes, round caps, currentColor.

const paths: Record<string, string> = {
  back: "M10 3.5 5.5 8l4.5 4.5",
  forward: "M6 3.5 10.5 8 6 12.5",
  reload: "M13 8a5 5 0 1 1-1.46-3.54M13 3v2.5h-2.5",
  close: "M4.5 4.5l7 7M11.5 4.5l-7 7",
  plus: "M8 3.5v9M3.5 8h9",
  star: "M8 2.6l1.6 3.3 3.6.5-2.6 2.5.6 3.6L8 10.8l-3.2 1.7.6-3.6L2.8 6.4l3.6-.5z",
  download: "M8 2.5v7.5M4.75 7 8 10.25 11.25 7M3 13.25h10",
  more: "M3.75 8h.01M8 8h.01M12.25 8h.01",
  lock: "M4.5 7.25h7v5.5h-7zM5.75 7.25V5.5a2.25 2.25 0 0 1 4.5 0v1.75",
  warning: "M8 2.75 14 13H2zM8 6.5v3M8 11.25h.01",
  globe: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11zM2.5 8h11M8 2.5c1.5 1.6 2.25 3.4 2.25 5.5S9.5 11.9 8 13.5M8 2.5C6.5 4.1 5.75 5.9 5.75 8S6.5 11.9 8 13.5",
  search: "M7 2.75a4.25 4.25 0 1 0 0 8.5 4.25 4.25 0 0 0 0-8.5zM10.1 10.1l3.15 3.15",
  minimize: "M3.5 8h9",
  maximize: "M4 4h8v8H4z",
  restore: "M4 6h6v6H4zM6 6V4h6v6h-2",
  tab: "M2.5 4.5h11v8h-11zM2.5 7h11",
  bookmark: "M4.5 2.75h7v10.5L8 10.75l-3.5 2.5z",
  history: "M2.75 8a5.25 5.25 0 1 0 1.54-3.71M2.5 2.75v2.5H5M8 5.25V8l2 1.5",
  folder: "M2.5 4.25h4l1.25 1.5h5.75v6.5h-11z",
  file: "M4.25 2.5h4.5L11.75 5.5v8h-7.5zM8.5 2.5v3.25h3.25",
  settings:
    "M8 5.75a2.25 2.25 0 1 0 0 4.5 2.25 2.25 0 0 0 0-4.5zM8 1.75v1.5M8 12.75v1.5M3.6 3.6l1.05 1.05M11.35 11.35l1.05 1.05M1.75 8h1.5M12.75 8h1.5M3.6 12.4l1.05-1.05M11.35 4.65l1.05-1.05",
  chevronRight: "M6.5 4 10.5 8l-4 4",
  chevronDown: "M4 6.5 8 10.5l4-4",
  trash: "M3 4.5h10M6.25 4.5V3h3.5v1.5M4.5 4.5l.6 8.75h5.8l.6-8.75",
  edit: "M10.5 2.75l2.75 2.75-7.5 7.5H3v-2.75z",
  external: "M9 2.75h4.25V7M13.25 2.75 7.5 8.5M11.5 9.5v3.75h-8.75V4.5H6.5",
  check: "M3.25 8.25 6.5 11.5l6.25-7",
  pause: "M5.5 3.5v9M10.5 3.5v9",
  play: "M5 3.25v9.5L12.5 8z",
  import: "M8 2.5v7M5 6.75 8 9.75l3-3M2.75 10.5v2.75h10.5V10.5",
  shield: "M8 2.25 3 4.25v3.5c0 3 2.25 5.25 5 6 2.75-.75 5-3 5-6v-3.5z",
  key: "M10 2.75a3.25 3.25 0 1 0 0 6.5 3.25 3.25 0 0 0 0-6.5zM7.75 8.25 2.75 13.25M4.5 11.5l1.5 1.5",
  user: "M8 2.75a2.75 2.75 0 1 0 0 5.5 2.75 2.75 0 0 0 0-5.5zM3 13.5c.75-2.25 2.75-3.5 5-3.5s4.25 1.25 5 3.5",
  sun: "M8 5.25a2.75 2.75 0 1 0 0 5.5 2.75 2.75 0 0 0 0-5.5zM8 1.5v1.25M8 13.25v1.25M1.5 8h1.25M13.25 8h1.25M3.4 3.4l.9.9M11.7 11.7l.9.9M3.4 12.6l.9-.9M11.7 4.3l.9-.9",
  moon: "M12.75 9.75A5.25 5.25 0 0 1 6.25 3.25a5.25 5.25 0 1 0 6.5 6.5z",
  monitor: "M2.5 3.25h11v7.5h-11zM6 13.25h4M8 10.75v2.5",
  info: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11zM8 7.25v3.5M8 5.25h.01",
  layers: "M8 2.5 2.5 5.25 8 8l5.5-2.75zM2.5 8 8 10.75 13.5 8M2.5 10.75 8 13.5l5.5-2.75",
  app: "M3 3h4v4H3zM9 3h4v4H9zM3 9h4v4H3zM9 9h4v4H9z",
  link: "M6.75 9.25a2.5 2.5 0 0 0 3.54 0l2-2a2.5 2.5 0 0 0-3.54-3.54l-.75.75M9.25 6.75a2.5 2.5 0 0 0-3.54 0l-2 2a2.5 2.5 0 0 0 3.54 3.54l.75-.75",
  home: "M2.75 7.25 8 2.75l5.25 4.5v6h-3.5v-3.5h-3.5v3.5h-3.5z",
  incognito:
    "M1.75 7.5h12.5M3.75 7.5 5 3h6l1.25 4.5M2.75 11a2 2 0 1 0 4 0 2 2 0 0 0-4 0zM9.25 11a2 2 0 1 0 4 0 2 2 0 0 0-4 0zM6.75 10.75c.75-.5 1.75-.5 2.5 0",
};

export type IconName = keyof typeof paths;

export function Icon({ name, size = 16, className, filled }: { name: IconName; size?: number; className?: string; filled?: boolean }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill={filled ? "currentColor" : "none"}
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      <path d={paths[name]} />
    </svg>
  );
}

export function Spinner({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" className="spinner" aria-hidden="true">
      <circle cx="8" cy="8" r="6" fill="none" stroke="currentColor" strokeOpacity="0.2" strokeWidth="1.75" />
      <path d="M8 2a6 6 0 0 1 6 6" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" />
    </svg>
  );
}

export function Logo({ size = 20 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
      <path d="M4.5 4.75h7l-7 6.5h7" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function Favicon({ src, loading, size = 16 }: { src: string | null | undefined; loading?: boolean; size?: number }) {
  if (loading) return <Spinner size={size - 2} />;
  if (src) return <img className="favicon" src={src} width={size} height={size} alt="" draggable={false} />;
  return <Icon name="globe" size={size} className="favicon-fallback" />;
}
