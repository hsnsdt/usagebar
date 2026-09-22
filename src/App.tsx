import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Dashboard from "./views/Dashboard";
import SettingsView from "./views/Settings";
import Welcome from "./views/Welcome";
import About from "./views/About";
import { hidePopup, useUsage } from "./hooks/useUsage";
import { setLanguage, t, useLang } from "./i18n";
import type { Settings, Theme } from "./types";
import "./styles/tokens.css";
import "./styles/app.css";

type View = "dashboard" | "settings" | "welcome" | "about";

export default function App() {
  useLang();
  const { snapshot, now } = useUsage();
  const [view, setView] = useState<View>("dashboard");
  const [enterKey, setEnterKey] = useState(0);

  // Theme + language from settings.
  useEffect(() => {
    invoke<Settings>("get_settings")
      .then((s) => applySettings(s))
      .catch(() => {});
    invoke<boolean>("is_first_run")
      .then((first) => first && setView("welcome"))
      .catch(() => {});
  }, []);

  // Tray menu "Settings" and re-play of the enter animation on each open.
  useEffect(() => {
    const unNav = listen<string>("navigate", (e) => {
      if (e.payload === "settings" || e.payload === "dashboard" || e.payload === "about") setView(e.payload);
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
        else if (view === "about") setView("settings");
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
            applySettings(s);
            setView("dashboard");
          }}
        />
      ) : view === "about" ? (
        <About onBack={() => setView("settings")} />
      ) : view === "settings" ? (
        <SettingsView
          onBack={() => setView("dashboard")}
          onSaved={(s) => applySettings(s)}
          onAbout={() => setView("about")}
        />
      ) : snapshot ? (
        <Dashboard snapshot={snapshot} now={now} onOpenSettings={() => setView("settings")} />
      ) : (
        <div className="center-note">{t("loading")}</div>
      )}
    </div>
  );
}

function applySettings(s: Settings) {
  applyTheme(s.theme);
  setLanguage(s.language);
}

function applyTheme(theme: Theme) {
  document.documentElement.setAttribute("data-theme", theme);
}
