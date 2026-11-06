import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "@/app/App";
import { installErrorLogging } from "@/lib/log";
import "./styles.css";

installErrorLogging();

// E2E builds only (vite --mode e2e, T-033): the WebdriverIO helper plugin.
if (import.meta.env.MODE === "e2e") await import("@wdio/tauri-plugin");

const root = document.getElementById("root");
if (!root) throw new Error("Root element #root is missing from index.html");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
