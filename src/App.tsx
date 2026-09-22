import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Dashboard from "./views/Dashboard";
import SettingsView from "./views/Settings";
import Welcome from "./views/Welcome";
import { hidePopup, useUsage } from "./hooks/useUsage";
import type { Settings, Theme } from "./types";
import "./styles/tokens.css";
import "./styles/app.css";

type View = "dashboard" | "settings" | "welcome";

export default function App() {
  const { snapshot, now } = useUsage();
  const [view, setView] = useState<View>("dashboard");
  const [enterKey, setEnterKey] = useState(0);

  // Theme from settings (system | dark | light).
  useEffect(() => {
    invoke<Settings>("get_settings")
      .then((s) => applyTheme(s.theme))
      .catch(() => {});
    invoke<boolean>("is_first_run")
      .then((first) => first && setView("welcome"))
      .catch(() => {});
  }, []);

  // Tray menu "Ayarlar" and re-play of the enter animation on each open.
  useEffect(() => {
    const unNav = listen<string>("navigate", (e) => {
      if (e.payload === "settings" || e.payload === "dashboard") setView(e.payload);
    });
    const unShown = listen("popup-shown", () => {
      setEnterKey((k) => k + 1);
    });
    return () => {
      unNav.then((f) => f()).catch(() => {});
      unShown.then((f) => f()).catch(() => {});
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (view === "settings") setView("dashboard");
        else if (view !== "welcome") hidePopup();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view]);

  return (
    <div className="shell" key={enterKey} onContextMenu={(e) => e.preventDefault()}>
      {view === "welcome" ? (
        <Welcome
          onDone={(s) => {
            applyTheme(s.theme);
            setView("dashboard");
          }}
        />
      ) : view === "settings" ? (
        <SettingsView onBack={() => setView("dashboard")} onSaved={(s) => applyTheme(s.theme)} />
      ) : snapshot ? (
        <Dashboard snapshot={snapshot} now={now} onOpenSettings={() => setView("settings")} />
      ) : (
        <div className="settings__loading">Yükleniyor…</div>
      )}
    </div>
  );
}

function applyTheme(theme: Theme) {
  document.documentElement.setAttribute("data-theme", theme);
}
