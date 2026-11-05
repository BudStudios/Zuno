// Finds the newest Windows installer in public/download/ and serves it as an attachment during
// `npm run dev`; Vite copies it to dist/download/ on build with the rest of public/. The page reads
// its name, version and size from __INSTALLER__, so dropping a new Zuno_x.y.z_x64-setup.exe into
// public/download/ is all a release needs.

import { createReadStream, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const versionOf = (file) => file.match(/(\d+)\.(\d+)\.(\d+)/)?.slice(1).map(Number) ?? [0, 0, 0];

function newest(root) {
  let files = [];
  try {
    files = readdirSync(root).filter((f) => /\.(exe|msi)$/i.test(f));
  } catch {
    return null;
  }
  if (files.length === 0) return null;
  files.sort((a, b) => {
    const va = versionOf(a);
    const vb = versionOf(b);
    return vb[0] - va[0] || vb[1] - va[1] || vb[2] - va[2];
  });
  const file = files[0];
  return {
    file,
    version: versionOf(file).join("."),
    bytes: statSync(join(root, file)).size,
    href: `download/${file}`,
  };
}

export function installer(dir = "public/download") {
  const root = resolve(dir);
  const info = newest(root);

  return {
    name: "zuno-installer",
    config() {
      return { define: { __INSTALLER__: JSON.stringify(info) } };
    },
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const path = decodeURIComponent((req.url ?? "").split("?")[0]);
        if (!info || !path.endsWith(`/${info.href}`)) return next();
        res.setHeader("Content-Type", "application/octet-stream");
        res.setHeader("Content-Length", String(info.bytes));
        res.setHeader("Content-Disposition", `attachment; filename="${info.file}"`);
        createReadStream(join(root, info.file)).pipe(res);
      });
    },
  };
}
