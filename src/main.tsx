import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import Mini from "./views/Mini";

// Same bundle for both windows; the label picks the UI.
const isMini = getCurrentWindow().label === "mini";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>{isMini ? <Mini /> : <App />}</React.StrictMode>,
);
