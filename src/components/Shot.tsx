// Product imagery. Every image is either the original screenshot from uploads/ or a capture of
// Zuno's real interface (app/dist) — never a mock-up. WebP files ship at 1x and 2x.

const base = import.meta.env.BASE_URL;

/** Intrinsic 1x sizes of the captures in public/screens. */
export const SHOTS = {
  "zuno-newtab": { w: 1408, h: 736, ext: "png", retina: false },
  newtab: { w: 1440, h: 860 },
  "quick-switcher": { w: 1440, h: 860 },
  "settings-privacy": { w: 1440, h: 860 },
  history: { w: 1440, h: 860 },
  bookmarks: { w: 1440, h: 860 },
  downloads: { w: 1440, h: 860 },
  import: { w: 1440, h: 860 },
  incognito: { w: 1440, h: 860 },
  "auth-review": { w: 1440, h: 860 },
  "part-quick": { w: 600, h: 340 },
  "part-chrome": { w: 1680, h: 115 },
  "part-auth": { w: 520, h: 367 },
  "part-incognito": { w: 580, h: 205 },
  "part-incognito-badge": { w: 95, h: 24 },
  "part-privacy": { w: 716, h: 309 },
  "part-downloads": { w: 796, h: 217 },
} satisfies Record<string, { w: number; h: number; ext?: string; retina?: boolean }>;

export type ShotName = keyof typeof SHOTS;

export function shotUrl(name: string, retina = false) {
  return `${base}screens/${name}${retina ? "@2x" : ""}.webp`;
}

export function Shot({
  name,
  alt,
  className,
  eager,
  light,
}: {
  name: ShotName;
  alt: string;
  className?: string;
  eager?: boolean;
  /** Use the light-theme capture of this view. */
  light?: boolean;
}) {
  const s: { w: number; h: number; ext?: string; retina?: boolean } = SHOTS[name];
  const file = light ? `${name}-light` : name;
  const src = s.ext ? `${base}screens/${file}.${s.ext}` : shotUrl(file);
  const srcSet = s.retina === false ? undefined : `${shotUrl(file)} 1x, ${shotUrl(file, true)} 2x`;
  return (
    <img
      className={className}
      src={src}
      srcSet={srcSet}
      width={s.w}
      height={s.h}
      alt={alt}
      loading={eager ? "eager" : "lazy"}
      decoding="async"
      draggable={false}
    />
  );
}
