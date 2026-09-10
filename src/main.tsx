import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

// Fonts are bundled locally (no CDN: the launcher may start offline).
import "@fontsource/inter/400.css";
import "@fontsource/inter/500.css";
import "@fontsource/inter/600.css";
import "@fontsource/inter/700.css";
import "@fontsource/space-grotesk/700.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "material-symbols/outlined.css";
import "./styles/index.css";

import { App } from "./app/App";
import { AppProvider } from "./store/AppStore";
import { logger } from "./lib/logger";

window.addEventListener("error", (event) => logger.error(`uncaught: ${event.message}`));
window.addEventListener("unhandledrejection", (event) => {
  const reason = event.reason as { message?: string; code?: string } | undefined;
  logger.error(`unhandled rejection: ${reason?.code ? `[${reason.code}] ` : ""}${reason?.message ?? String(event.reason)}`);
});
// The launcher chrome has no context menu; text areas keep the native one.
document.addEventListener("contextmenu", (event) => {
  const target = event.target as HTMLElement | null;
  if (!target?.closest("input, textarea, .selectable, .rich-content, .console")) event.preventDefault();
});

const root = document.getElementById("root");
if (!root) throw new Error("#root is missing");

createRoot(root).render(
  <StrictMode>
    <AppProvider>
      <App />
    </AppProvider>
  </StrictMode>,
);
