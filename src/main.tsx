import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles/app.css";

// Zuno's interface is not a document: suppress the default webview context menu except in
// editable fields, where cut/copy/paste are useful.
document.addEventListener("contextmenu", (e) => {
  const el = e.target as HTMLElement | null;
  if (!el?.closest("input, textarea, [contenteditable=true], .page")) e.preventDefault();
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
