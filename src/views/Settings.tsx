import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { BackIcon } from "../components/Icons";
import { pct } from "../format";
import { setLanguage, t, useLang, type LangSetting } from "../i18n";
import { tokensK } from "../format";
import { MIN_POLL_INTERVAL_SEC, type Settings, type Snapshot, type Theme, type TimeFormat } from "../types";

const FIVE_HOUR_OPTIONS = [50, 75, 90];
const SEVEN_DAY_OPTIONS = [80, 95];

type Props = {
  onBack: () => void;
  onSaved: (s: Settings) => void;
  onAbout: () => void;
};

export default function SettingsView({ onBack, onSaved, onAbout }: Props) {
  useLang();
  const [s, setS] = useState<Settings | null>(null);
  const [version, setVersion] = useState("");
  const [detected, setDetected] = useState<number | null>(null);
  const [savedFlash, setSavedFlash] = useState(false);

  useEffect(() => {
    invoke<Settings>("get_settings").then(setS).catch(() => {});
    invoke<string>("app_version").then(setVersion).catch(() => {});
    invoke<Snapshot>("get_snapshot")
      .then((snap) => setDetected(snap.context?.usable ?? null))
      .catch(() => {});
  }, []);

  if (!s) return <div className="center-note">{t("loading")}</div>;

  const patch = (p: Partial<Settings>) => setS({ ...s, ...p });
  const patchN = (p: Partial<Settings["notifications"]>) =>
    setS({ ...s, notifications: { ...s.notifications, ...p } });

  const toggleIn = (list: number[], v: number) =>
    list.includes(v) ? list.filter((x) => x !== v) : [...list, v].sort((a, b) => a - b);

  const save = async () => {
    const clean: Settings = {
      ...s,
      pollIntervalSec: Math.max(MIN_POLL_INTERVAL_SEC, Math.round(s.pollIntervalSec || 0)),
      usableContextTokens: Math.max(10_000, Math.round(s.usableContextTokens || 0)),
    };
    const saved = await invoke<Settings>("save_settings", { settings: clean });
    setS(saved);
    onSaved(saved);
    setSavedFlash(true);
    window.setTimeout(() => setSavedFlash(false), 1200);
  };

  return (
    <>
      <header className="head head--sub">
        <div className="head__text">
          <div className="head__title">
            <button className="iconbtn iconbtn--back" title={t("back")} aria-label={t("back")} onClick={onBack}>
              <BackIcon />
            </button>
            <span>{t("settings")}</span>
          </div>
        </div>
        <div className="head__actions">
          <button className={`btn btn--primary${savedFlash ? " btn--ok" : ""}`} onClick={save}>
            {savedFlash ? t("saved") : t("save")}
          </button>
        </div>
      </header>

      <div className="stack">
        <div className="eyebrow eyebrow--group">{t("sGeneral")}</div>
        <section className="row-card row-card--list">
          <Row label={t("sPoll")} hint={t("sPollHint", { min: MIN_POLL_INTERVAL_SEC })}>
            <input
              className="input input--num"
              type="number"
              min={MIN_POLL_INTERVAL_SEC}
              step={30}
              value={s.pollIntervalSec}
              onChange={(e) => patch({ pollIntervalSec: Number(e.target.value) })}
              onBlur={() =>
                patch({ pollIntervalSec: Math.max(MIN_POLL_INTERVAL_SEC, Math.round(s.pollIntervalSec || 0)) })
              }
            />
            <span className="unit">{t("sSec")}</span>
          </Row>
          <Toggle
            label={t("sAutoContext")}
            hint={
              s.autoContextWindow && detected
                ? `${t("sAutoContextHint")} · ${tokensK(detected)}`
                : t("sAutoContextHint")
            }
            checked={s.autoContextWindow}
            onChange={(v) => patch({ autoContextWindow: v })}
          />
          {!s.autoContextWindow && (
            <Row label={t("sContext")} hint={t("sContextHint")}>
              <input
                className="input input--num input--wide"
                type="number"
                min={10000}
                step={5000}
                value={s.usableContextTokens}
                onChange={(e) => patch({ usableContextTokens: Number(e.target.value) })}
              />
              <span className="unit">{t("sToken")}</span>
            </Row>
          )}
          <Row label={t("sTheme")}>
            <select className="input" value={s.theme} onChange={(e) => patch({ theme: e.target.value as Theme })}>
              <option value="dark">{t("themeDark")}</option>
              <option value="light">{t("themeLight")}</option>
              <option value="system">{t("themeSystem")}</option>
            </select>
          </Row>
          <Row label={t("sLanguage")}>
            <select
              className="input"
              value={s.language}
              onChange={(e) => {
                const language = e.target.value as LangSetting;
                patch({ language });
                setLanguage(language);
              }}
            >
              <option value="system">{t("langSystem")}</option>
              <option value="tr">Türkçe</option>
              <option value="en">English</option>
            </select>
          </Row>
          <Row label={t("sTimeFormat")}>
            <select
              className="input"
              value={s.timeFormat}
              onChange={(e) => patch({ timeFormat: e.target.value as TimeFormat })}
            >
              <option value="system">{t("themeSystem")}</option>
              <option value="24h">{t("time24")}</option>
              <option value="12h">{t("time12")}</option>
            </select>
          </Row>
          <Toggle
            label={t("sShowRemaining")}
            hint={t("sShowRemainingHint")}
            checked={s.showRemaining}
            onChange={(v) => patch({ showRemaining: v })}
          />
        </section>

        <div className="eyebrow eyebrow--group">{t("sTray")}</div>
        <section className="row-card row-card--list">
          <Toggle
            label={t("sPercentText")}
            hint={t("sPercentHint")}
            checked={s.showPercentText}
            onChange={(v) => patch({ showPercentText: v })}
          />
          <Toggle
            label={t("sAutostart")}
            hint={t("sAutostartHint")}
            checked={s.startWithWindows}
            onChange={(v) => patch({ startWithWindows: v })}
          />
        </section>

        <div className="eyebrow eyebrow--group">{t("sNotifications")}</div>
        <section className="row-card row-card--list">
          <Toggle
            label={t("sNotifEnabled")}
            checked={s.notifications.enabled}
            onChange={(v) => patchN({ enabled: v })}
          />
          <Row label={t("sFiveThresholds")}>
            <div className="chips">
              {FIVE_HOUR_OPTIONS.map((p) => (
                <button
                  key={p}
                  className={`chip${s.notifications.fiveHour.includes(p) ? " chip--on" : ""}`}
                  disabled={!s.notifications.enabled}
                  onClick={() => patchN({ fiveHour: toggleIn(s.notifications.fiveHour, p) })}
                >
                  {pct(p)}
                </button>
              ))}
            </div>
          </Row>
          <Row label={t("sWeekThresholds")}>
            <div className="chips">
              {SEVEN_DAY_OPTIONS.map((p) => (
                <button
                  key={p}
                  className={`chip${s.notifications.sevenDay.includes(p) ? " chip--on" : ""}`}
                  disabled={!s.notifications.enabled}
                  onClick={() => patchN({ sevenDay: toggleIn(s.notifications.sevenDay, p) })}
                >
                  {pct(p)}
                </button>
              ))}
            </div>
          </Row>
          <Row label={t("sContextLow")} hint={t("sContextLowHint")}>
            <input
              className="input input--num input--wide"
              type="number"
              min={0}
              step={5000}
              disabled={!s.notifications.enabled}
              value={s.notifications.contextLowTokens}
              onChange={(e) => patchN({ contextLowTokens: Number(e.target.value) })}
            />
            <span className="unit">{t("sToken")}</span>
          </Row>
          <Toggle
            label={t("sOnReset")}
            hint={t("sOnResetHint")}
            checked={s.notifications.onReset}
            onChange={(v) => patchN({ onReset: v })}
          />
        </section>

        <div className="settings__foot">
          <button className="link" onClick={() => invoke("open_logs_dir")}>
            {t("sOpenLogs")}
          </button>
          <button className="link" onClick={onAbout}>
            {t("about")}
            {version && <span className="settings__version"> · v{version}</span>}
          </button>
          <button className="link link--danger" onClick={() => invoke("quit_app")}>
            {t("sQuit")}
          </button>
        </div>
      </div>
    </>
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="row">
      <div className="row__label">
        <span>{label}</span>
        {hint && <span className="row__hint">{hint}</span>}
      </div>
      <div className="row__ctl">{children}</div>
    </div>
  );
}

function Toggle({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="row row--toggle">
      <div className="row__label">
        <span>{label}</span>
        {hint && <span className="row__hint">{hint}</span>}
      </div>
      <span className={`switch${checked ? " switch--on" : ""}`}>
        <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
        <span className="switch__knob" />
      </span>
    </label>
  );
}
