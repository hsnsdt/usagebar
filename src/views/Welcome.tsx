import { useEffect, useState, type CSSProperties } from "react";
import { invoke } from "@tauri-apps/api/core";
import Ring from "../components/Ring";
import { setLanguage, t, useLang, type LangSetting } from "../i18n";
import type { Settings } from "../types";

type Props = { onDone: (s: Settings) => void };

function TrayGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <rect x="3" y="4" width="18" height="14" rx="2" />
      <path d="M3 15h18M8 21h8" />
    </svg>
  );
}
function ShieldGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M12 3l7 3v5c0 5-3.5 8.5-7 10-3.5-1.5-7-5-7-10V6z" />
      <path d="M9 12l2 2 4-4" />
    </svg>
  );
}
function BellGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M6 16V11a6 6 0 0 1 12 0v5l1.5 2h-15z" />
      <path d="M10 20a2 2 0 0 0 4 0" />
    </svg>
  );
}

/** First-run screen. Saving creates settings.json, which ends first-run mode. */
export default function Welcome({ onDone }: Props) {
  useLang();
  const [s, setS] = useState<Settings | null>(null);

  useEffect(() => {
    invoke<Settings>("get_settings").then(setS).catch(() => {});
  }, []);

  const start = async () => {
    if (!s) return;
    const saved = await invoke<Settings>("save_settings", { settings: s });
    onDone(saved);
  };

  return (
    <>
      <header className="head head--sub">
        <div className="head__text">
          <div className="head__title">
            <span>UsageTray</span>
          </div>
        </div>
        <div className="head__actions">
          {s && (
            <select
              className="input input--lang"
              value={s.language}
              aria-label={t("sLanguage")}
              onChange={(e) => {
                const language = e.target.value as LangSetting;
                setS({ ...s, language });
                setLanguage(language);
              }}
            >
              <option value="system">{t("langSystem")}</option>
              <option value="tr">Türkçe</option>
              <option value="en">English</option>
            </select>
          )}
        </div>
      </header>

      <div className="stack">
        <section className="hero hero--welcome" style={{ "--tone": "var(--green)" } as CSSProperties}>
          <Ring size={84} stroke={9} value={38} tone="green" marker={30}>
            <span className="ring__pct ring__pct--sm tone-text-green">38</span>
            <span className="ring__unit">%</span>
          </Ring>
          <div className="hero__text">
            <div className="hero__big hero__big--sm">{t("wTitle")}</div>
            <div className="hero__sub">{t("wLead")}</div>
          </div>
        </section>

        <section className="row-card row-card--list">
          <Feature icon={<TrayGlyph />} title={t("wTrayTitle")} body={t("wTrayBody")} />
          <Feature icon={<ShieldGlyph />} title={t("wPrivacyTitle")} body={t("wPrivacyBody")} />
          <Feature icon={<BellGlyph />} title={t("wNotifyTitle")} body={t("wNotifyBody")} />
        </section>

        {s && (
          <section className="row-card row-card--list">
            <label className="row row--toggle">
              <div className="row__label">
                <span>{t("sAutostart")}</span>
                <span className="row__hint">{t("sAutostartHint")}</span>
              </div>
              <span className={`switch${s.startWithWindows ? " switch--on" : ""}`}>
                <input
                  type="checkbox"
                  checked={s.startWithWindows}
                  onChange={(e) => setS({ ...s, startWithWindows: e.target.checked })}
                />
                <span className="switch__knob" />
              </span>
            </label>
          </section>
        )}
      </div>

      <footer className="foot foot--welcome">
        <button className="btn btn--accent btn--block" disabled={!s} onClick={start}>
          {t("wStart")}
        </button>
      </footer>
    </>
  );
}

function Feature({ icon, title, body }: { icon: React.ReactNode; title: string; body: string }) {
  return (
    <div className="feature">
      <span className="feature__icon">{icon}</span>
      <div className="feature__text">
        <div className="feature__title">{title}</div>
        <div className="feature__body">{body}</div>
      </div>
    </div>
  );
}
