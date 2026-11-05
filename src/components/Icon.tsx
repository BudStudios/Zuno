// Zuno's own icon set (app/src/components/Icon.tsx): 16px grid, 1.5px strokes, round caps,
// currentColor. "menu" and "copy" are drawn on the same grid for the website.

const paths = {
  search: "M7 2.75a4.25 4.25 0 1 0 0 8.5 4.25 4.25 0 0 0 0-8.5zM10.1 10.1l3.15 3.15",
  close: "M4.5 4.5l7 7M11.5 4.5l-7 7",
  check: "M3.25 8.25 6.5 11.5l6.25-7",
  download: "M8 2.5v7.5M4.75 7 8 10.25 11.25 7M3 13.25h10",
  import: "M8 2.5v7M5 6.75 8 9.75l3-3M2.75 10.5v2.75h10.5V10.5",
  shield: "M8 2.25 3 4.25v3.5c0 3 2.25 5.25 5 6 2.75-.75 5-3 5-6v-3.5z",
  key: "M10 2.75a3.25 3.25 0 1 0 0 6.5 3.25 3.25 0 0 0 0-6.5zM7.75 8.25 2.75 13.25M4.5 11.5l1.5 1.5",
  lock: "M4.5 7.25h7v5.5h-7zM5.75 7.25V5.5a2.25 2.25 0 0 1 4.5 0v1.75",
  history: "M2.75 8a5.25 5.25 0 1 0 1.54-3.71M2.5 2.75v2.5H5M8 5.25V8l2 1.5",
  file: "M4.25 2.5h4.5L11.75 5.5v8h-7.5zM8.5 2.5v3.25h3.25",
  link: "M6.75 9.25a2.5 2.5 0 0 0 3.54 0l2-2a2.5 2.5 0 0 0-3.54-3.54l-.75.75M9.25 6.75a2.5 2.5 0 0 0-3.54 0l-2 2a2.5 2.5 0 0 0 3.54 3.54l.75-.75",
  layers: "M8 2.5 2.5 5.25 8 8l5.5-2.75zM2.5 8 8 10.75 13.5 8M2.5 10.75 8 13.5l5.5-2.75",
  tab: "M2.5 4.5h11v8h-11zM2.5 7h11",
  globe: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11zM2.5 8h11M8 2.5c1.5 1.6 2.25 3.4 2.25 5.5S9.5 11.9 8 13.5M8 2.5C6.5 4.1 5.75 5.9 5.75 8S6.5 11.9 8 13.5",
  info: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11zM8 7.25v3.5M8 5.25h.01",
  sun: "M8 5.25a2.75 2.75 0 1 0 0 5.5 2.75 2.75 0 0 0 0-5.5zM8 1.5v1.25M8 13.25v1.25M1.5 8h1.25M13.25 8h1.25M3.4 3.4l.9.9M11.7 11.7l.9.9M3.4 12.6l.9-.9M11.7 4.3l.9-.9",
  moon: "M12.75 9.75A5.25 5.25 0 0 1 6.25 3.25a5.25 5.25 0 1 0 6.5 6.5z",
  app: "M3 3h4v4H3zM9 3h4v4H9zM3 9h4v4H3zM9 9h4v4H9z",
  settings:
    "M8 5.75a2.25 2.25 0 1 0 0 4.5 2.25 2.25 0 0 0 0-4.5zM8 1.75v1.5M8 12.75v1.5M3.6 3.6l1.05 1.05M11.35 11.35l1.05 1.05M1.75 8h1.5M12.75 8h1.5M3.6 12.4l1.05-1.05M11.35 4.65l1.05-1.05",
  incognito:
    "M1.75 7.5h12.5M3.75 7.5 5 3h6l1.25 4.5M2.75 11a2 2 0 1 0 4 0 2 2 0 0 0-4 0zM9.25 11a2 2 0 1 0 4 0 2 2 0 0 0-4 0zM6.75 10.75c.75-.5 1.75-.5 2.5 0",
  menu: "M2.75 4.5h10.5M2.75 8h10.5M2.75 11.5h10.5",
  copy: "M5.75 5.75h7v7h-7zM3.25 10.25v-7h7",
  arrow: "M3 8h10M9 4l4 4-4 4",
  windows: "M2.75 3.75 7.25 3.1v4.4h-4.5zM8.5 2.9l4.75-.65V7.5H8.5zM2.75 8.75h4.5v4.15l-4.5-.65zM8.5 8.75h4.75v5l-4.75-.65z",
} as const;

export type IconName = keyof typeof paths;

export function Icon({ name, size = 16, className }: { name: IconName; size?: number; className?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
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

/** GitHub's mark. Filled, so it lives outside the stroke-based set above. */
export function GitHubMark({ size = 16, className }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="currentColor" className={className} aria-hidden="true">
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z" />
    </svg>
  );
}

/** The Zuno mark, exactly as drawn in the app. */
export function Logo({ size = 20, className }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" className={className} aria-hidden="true">
      <path d="M4.5 4.75h7l-7 6.5h7" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
