# Zuno website

The landing page for **Zuno — Less noise. More web.** It is a separate project from the browser in
`../app` and does not change it.

## Run it

```powershell
npm install
npm run dev        # http://localhost:5180
npm run build      # type-checks, then writes the static site to dist/
npm run preview    # serves dist/ on http://localhost:5181
```

`dist/` uses relative paths, so it can be hosted from any folder or sub-path (GitHub Pages,
Netlify, a plain web server).

## Stack

Vite, React 19 and TypeScript, the same toolchain and versions as Zuno's interface. No UI
libraries, no web fonts: the page uses Zuno's own type (Segoe UI Variable, Cascadia Mono), colour
tokens and icon set, copied from `app/src/styles/app.css` and `app/src/components/Icon.tsx`.

```
src/
  App.tsx              page composition, Ctrl/⌘ K shortcut, toast
  lib.ts               section list (nav, footer, quick switcher), helpers
  styles.css           design tokens and all styles
  components/          one file per section, plus Icon, Shot and the page quick switcher (Palette)
public/screens/        product imagery (WebP at 1x and 2x)
tools/screenshots/     scripts that regenerate the imagery from the real app
```

## Product imagery

- `public/screens/zuno-newtab.png` is the original screenshot from `../uploads/image.png`, unchanged.
- Every other image is a capture of **Zuno's real interface**: the built UI in `app/dist`, rendered
  in Chrome with the Tauri bridge replaced by demo data (`tools/screenshots/mock.js`). The
  components, styles and text are the app's own. The tabs, history, bookmarks and downloads shown
  are sample data, and site icons are neutral letter tiles rather than real logos.

To regenerate them after the app's UI changes (needs Google Chrome, plus `npm i --no-save playwright-core`
in this folder):

```powershell
cd ../app; npm run build; cd ../website
node tools/screenshots/serve.mjs ../app/dist 4310          # leave running
node tools/screenshots/capture.mjs http://localhost:4310/ shots
$env:LIGHT=1; node tools/screenshots/capture.mjs http://localhost:4310/ shots; Remove-Item Env:LIGHT
node tools/screenshots/parts.mjs http://localhost:4310/ shots public/screens
```

`capture.mjs` takes full-window shots, `parts.mjs` takes element crops (quick switcher, tab strip,
dialogs) and converts everything to WebP.

## Downloads

The download card, the hero button and the page quick switcher all point at the newest installer in
`public/download/` (highest `x.y.z` in the file name). `tools/installer.mjs` serves it during
`npm run dev` and passes its name, version and size to the page; Vite copies the folder to
`dist/download/` on build. To ship a release, drop `Zuno_x.y.z_x64-setup.exe` into `public/download/`
and rebuild. Every file in that folder is published, so remove old installers you don't want online.

## Gradient textures

`public/bg/dusk.jpg` (hero mat, download card) and `public/bg/ember.jpg` (sign-in feature stage) were
made in Gradients Studio from Zuno's palette. They are the 960px previews; to use full-size exports,
render the same settings at 2400×1350 and keep the file names.

- dusk: blobs, background `#0e0f12`, colours `#6f62b8 #16171c #3f6f86 #1a1a22 #7a4f8a`, seed 2290117731, blur 880, grain 0.24, saturation 112
- ember: blobs, background `#111214`, colours `#5f8a62 #18181c #9a6656 #1a1b1f #b08c5c`, seed 3012449871, blur 880, grain 0.24, saturation 108
