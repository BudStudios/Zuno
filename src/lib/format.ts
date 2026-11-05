export function isInternal(url: string): boolean {
  return url.startsWith("zuno://");
}

export function internalPage(url: string): string | null {
  if (!isInternal(url)) return null;
  return url.slice("zuno://".length).split(/[/?#]/)[0].toLowerCase() || "newtab";
}

export function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return "";
  }
}

/** Address shown in the unfocused address bar: scheme and trailing slash trimmed. */
export function displayUrl(url: string): { host: string; rest: string } {
  try {
    const u = new URL(url);
    if (u.protocol === "http:" || u.protocol === "https:") {
      const host = u.host.replace(/^www\./, "");
      let rest = u.pathname + u.search + u.hash;
      if (rest === "/") rest = "";
      return { host, rest };
    }
  } catch {
    /* fall through */
  }
  return { host: "", rest: url };
}

export type Security = "secure" | "insecure" | "internal" | "file" | "other";

export function securityOf(url: string): Security {
  if (url.startsWith("https://")) return "secure";
  if (url.startsWith("http://")) {
    const host = hostOf(url).split(":")[0];
    return host === "localhost" || host === "127.0.0.1" || host === "[::1]" ? "other" : "insecure";
  }
  if (isInternal(url)) return "internal";
  if (url.startsWith("file:")) return "file";
  return "other";
}

export function formatBytes(n: number | null | undefined): string {
  if (n == null || n < 0) return "";
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

export function formatTime(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
}

export function dayLabel(ms: number): string {
  const d = new Date(ms);
  const today = new Date();
  const startOf = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((startOf(today) - startOf(d)) / 86_400_000);
  if (diff === 0) return "Today";
  if (diff === 1) return "Yesterday";
  return d.toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric", year: d.getFullYear() === today.getFullYear() ? undefined : "numeric" });
}

export function relativeDate(ms: number): string {
  const d = new Date(ms);
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
}
