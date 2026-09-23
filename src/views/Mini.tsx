import { useEffect, type MouseEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { applySettings } from "../App";
import { CloseIcon } from "../components/Icons";
import Ring from "../components/Ring";
import { isNow, parseDate, pct, remainingTr, shown, toneFor } from "../format";
import { useUsage } from "../hooks/useUsage";
import { t, useLang } from "../i18n";
import type { Settings } from "../types";
import "../styles/tokens.css";
import "../styles/app.css";

const FIVE_HOURS_MS = 5 * 60 * 60 * 1000;

/** Always-on-top mini gauge. Drag anywhere; double-click opens the panel. */
export default function Mini() {
  useLang();
  const { snapshot, now } = useUsage();

  useEffect(() => {
    document.documentElement.classList.add("is-mini");
    invoke<Settings>("get_settings").then(applySettings).catch(() => {});
    const un = listen<Settings>("settings-changed", (e) => applySettings(e.payload));
    return () => {
      un.then((f) => f()).catch(() => {});
    };
  }, []);

  const onMouseDown = (e: MouseEvent) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
    // Double-click: the second press opens the panel instead of dragging.
    if (e.detail === 2) {
      invoke("open_popup").catch(() => {});
      return;
    }
    getCurrentWindow().startDragging().catch(() => {});
  };

  const hidden = !snapshot || snapshot.status === "no_credentials";
  const five = hidden ? null : snapshot.fiveHour;
  const week = hidden ? null : snapshot.sevenDay;
  const stale = snapshot?.stale ?? true;
  const tone = five ? toneFor(five.utilization) : "gray";
  const resetsAt = parseDate(five?.resetsAt ?? null);
  const remaining = resetsAt ? remainingTr(resetsAt, now) : null;
  const marker = resetsAt
    ? Math.max(0, Math.min(100, ((FIVE_HOURS_MS - (resetsAt.getTime() - now.getTime())) / FIVE_HOURS_MS) * 100))
    : null;

  return (
    <div className="mini" onMouseDown={onMouseDown} onContextMenu={(e) => e.preventDefault()} title={t("miniOpen")}>
      <Ring size={46} stroke={5} value={five?.utilization ?? 0} tone={tone} marker={marker} muted={stale}>
        <span className={`mini__pct tone-text-${tone}`}>{five ? Math.round(shown(five.utilization)) : "—"}</span>
      </Ring>
      <div className="mini__text">
        <div className="mini__main">
          {remaining ? (isNow(remaining) ? t("resetting") : remaining) : t("noData")}
        </div>
        <div className="mini__sub">
          {t("fiveHour")}
          {week && (
            <>
              {" · "}
              {t("miniWeek")} <span className={`tone-text-${toneFor(week.utilization)}`}>{pct(shown(week.utilization))}</span>
            </>
          )}
        </div>
      </div>
      <button
        className="mini__close"
        title={t("miniClose")}
        aria-label={t("miniClose")}
        onClick={() => invoke("set_mini_window", { on: false })}
      >
        <CloseIcon size={11} />
      </button>
    </div>
  );
}
