import { useState, type CSSProperties } from "react";
import ContextMeter from "../components/ContextMeter";
import { GearIcon, RefreshIcon } from "../components/Icons";
import Ring from "../components/Ring";
import StatusBanner from "../components/StatusBanner";
import UsageBar from "../components/UsageBar";
import WeekCard from "../components/WeekCard";
import {
  agoTr,
  isNow,
  isShowingRemaining,
  localClockTr,
  money,
  parseDate,
  pct,
  remainingTr,
  shown,
  toneFor,
  whenLabel,
} from "../format";
import { refreshNow } from "../hooks/useUsage";
import { t, useLang } from "../i18n";
import type { ScopedSnap, Snapshot, SpendSnap, WindowSnap } from "../types";

const FIVE_HOURS_MS = 5 * 60 * 60 * 1000;
const SEVEN_DAYS_MS = 7 * 24 * 60 * 60 * 1000;

type Props = {
  snapshot: Snapshot;
  now: Date;
  onOpenSettings: () => void;
};

export default function Dashboard({ snapshot, now, onOpenSettings }: Props) {
  useLang();
  const [spinning, setSpinning] = useState(false);
  const apiDataHidden = snapshot.status === "no_credentials";

  const onRefresh = () => {
    setSpinning(true);
    refreshNow().finally(() => window.setTimeout(() => setSpinning(false), 900));
  };

  return (
    <>
      <header className="head">
        <div className="head__text">
          <div className="head__title">
            <span>Claude</span>
            {snapshot.plan && <span className="badge">{snapshot.plan.toUpperCase()}</span>}
          </div>
          <StatusLine snapshot={snapshot} now={now} />
        </div>
        <div className="head__actions">
          <button className={`iconbtn${spinning ? " spin" : ""}`} title={t("refresh")} aria-label={t("refresh")} onClick={onRefresh}>
            <RefreshIcon size={16} />
          </button>
          <button className="iconbtn" title={t("settings")} aria-label={t("settings")} onClick={onOpenSettings}>
            <GearIcon size={17} />
          </button>
        </div>
      </header>

      <StatusBanner snapshot={snapshot} />

      <div className="stack">
        {!apiDataHidden && (
          <>
            <Hero window={snapshot.fiveHour} now={now} stale={snapshot.stale} />
            <WeeklyRow window={snapshot.sevenDay} scoped={snapshot.scoped ?? []} now={now} stale={snapshot.stale} />
            {snapshot.spend && <SpendRow spend={snapshot.spend} stale={snapshot.stale} />}
          </>
        )}
        <ContextMeter context={snapshot.context} />
        <WeekCard week={snapshot.week} />
      </div>
    </>
  );
}

// ------------------------------------------------------------------ header

function StatusLine({ snapshot, now }: { snapshot: Snapshot; now: Date }) {
  let dot = "dot--ok";
  let text = agoTr(snapshot.lastUpdated, now);
  switch (snapshot.status) {
    case "ok":
      if (snapshot.stale) {
        dot = "dot--warn";
        text = `${t("statusStale")} · ${text}`;
      }
      break;
    case "rate_limited":
      dot = "dot--warn";
      text = t("statusRateLimited");
      break;
    case "offline":
      dot = "dot--warn";
      text = t("statusOffline");
      break;
    case "token_expired":
      dot = "dot--warn";
      text = t("statusTokenExpired");
      break;
    case "no_credentials":
      dot = "dot--bad";
      text = t("statusNoCreds");
      break;
    case "error":
      dot = "dot--bad";
      text = t("statusError");
      break;
  }
  return (
    <div className="head__status">
      <span className={`dot ${dot}`} />
      {text}
    </div>
  );
}

// -------------------------------------------------------------------- hero

/** Expected usage (%) at this point of a window of `windowMs`. */
function pace(resetsAt: Date | null, now: Date, windowMs: number): number | null {
  if (!resetsAt) return null;
  const elapsed = windowMs - (resetsAt.getTime() - now.getTime());
  return Math.max(0, Math.min(100, (elapsed / windowMs) * 100));
}

function paceLabel(util: number, expected: number | null): { text: string; cls: string } | null {
  if (expected == null) return null;
  const diff = util - expected;
  if (util >= 90) return { text: t("paceNearLimit"), cls: "pace--bad" };
  if (diff > 25) return { text: t("paceBurning"), cls: "pace--bad" };
  if (diff > 10) return { text: t("paceHigh"), cls: "pace--warn" };
  if (diff < -25) return { text: t("paceHeadroom"), cls: "pace--ok" };
  return { text: t("paceFine"), cls: "pace--ok" };
}

function Hero({ window: w, now, stale }: { window: WindowSnap | null; now: Date; stale: boolean }) {
  if (!w) {
    return (
      <section className="hero" style={{ "--tone": "var(--gray)" } as CSSProperties}>
        <Ring size={112} stroke={11} value={0} tone="gray" muted>
          <span className="ring__pct ring__pct--dim">—</span>
        </Ring>
        <div className="hero__text">
          <div className="eyebrow">{t("fiveHourWindow")}</div>
          <div className="hero__big">{t("noData")}</div>
        </div>
      </section>
    );
  }
  const tone = toneFor(w.utilization);
  const resetsAt = parseDate(w.resetsAt);
  const expected = pace(resetsAt, now, FIVE_HOURS_MS);
  const p = paceLabel(w.utilization, expected);
  const remaining = resetsAt ? remainingTr(resetsAt, now) : null;

  return (
    <section className="hero" style={{ "--tone": `var(--${tone})` } as CSSProperties}>
      <Ring size={112} stroke={11} value={w.utilization} tone={tone} marker={expected} muted={stale}>
        <span className={`ring__pct tone-text-${tone}`}>{Math.round(shown(w.utilization))}</span>
        <span className="ring__unit">%</span>
      </Ring>
      <div className="hero__text">
        <div className="eyebrow">
          {t("fiveHourWindow")}
          {isShowingRemaining() && ` · ${t("leftTag")}`}
        </div>
        {remaining && resetsAt ? (
          <>
            <div className="hero__big">{isNow(remaining) ? t("resetting") : remaining}</div>
            <div className="hero__sub">
              {isNow(remaining) ? t("newWindow") : t("untilReset", { clock: whenLabel(resetsAt, now) })}
            </div>
          </>
        ) : (
          <div className="hero__sub">{t("resetUnknown")}</div>
        )}
        {p && (
          <div className={`pace ${p.cls}`}>
            <span className="pace__dot" />
            {p.text}
          </div>
        )}
      </div>
    </section>
  );
}

// ------------------------------------------------------------------ weekly

type WeeklyProps = { window: WindowSnap | null; scoped: ScopedSnap[]; now: Date; stale: boolean };

function WeeklyRow({ window: w, scoped, now, stale }: WeeklyProps) {
  if (!w) {
    return (
      <section className="row-card">
        <div className="row-card__head">
          <div className="row-card__text">
            <div className="row-card__title">{t("weekly")}</div>
            <div className="row-card__sub">{t("noData")}</div>
          </div>
          <div className="row-card__value row-card__value--dim">—</div>
        </div>
        <UsageBar value={0} tone="gray" muted />
      </section>
    );
  }
  const tone = toneFor(w.utilization);
  const resetsAt = parseDate(w.resetsAt);
  const expected = pace(resetsAt, now, SEVEN_DAYS_MS);
  const sub = resetsAt
    ? t("weeklyResets", { clock: localClockTr(resetsAt), remaining: remainingTr(resetsAt, now) })
    : t("allModels");
  return (
    <section className="row-card">
      <div className="row-card__head">
        <div className="row-card__text">
          <div className="row-card__title">{t("weekly")}</div>
          <div className="row-card__sub">{sub}</div>
        </div>
        <div className={`row-card__value tone-text-${tone}`}>
          {pct(shown(w.utilization))}
          {isShowingRemaining() && <span className="row-card__unit"> {t("leftTag")}</span>}
        </div>
      </div>
      <UsageBar value={w.utilization} tone={tone} marker={expected} muted={stale} />
      {scoped.length > 0 && (
        <div className="scoped" aria-label={t("modelLimits")}>
          {scoped.map((l) => (
            <ScopedLine key={l.label} limit={l} now={now} stale={stale} />
          ))}
        </div>
      )}
    </section>
  );
}

function ScopedLine({ limit: l, now, stale }: { limit: ScopedSnap; now: Date; stale: boolean }) {
  const tone = toneFor(l.utilization);
  const resetsAt = parseDate(l.resetsAt);
  const title = resetsAt
    ? t("weeklyResets", { clock: localClockTr(resetsAt), remaining: remainingTr(resetsAt, now) })
    : undefined;
  return (
    <div className="scoped__line" title={title}>
      <span className="scoped__label">{l.label}</span>
      <UsageBar value={l.utilization} tone={tone} muted={stale} />
      <span className={`scoped__value tone-text-${tone}`}>{pct(shown(l.utilization))}</span>
    </div>
  );
}

// ------------------------------------------------------------ extra usage

function SpendRow({ spend: s, stale }: { spend: SpendSnap; stale: boolean }) {
  const percent = s.percent ?? (s.limit ? (s.used / s.limit) * 100 : null);
  const tone = percent == null ? "gray" : toneFor(percent);
  const sub = s.limit != null ? t("extraOf", { limit: money(s.limit, s.currency) }) : t("extraNoLimit");
  return (
    <section className="row-card">
      <div className="row-card__head">
        <div className="row-card__text">
          <div className="row-card__title">{t("extraUsage")}</div>
          <div className="row-card__sub">{sub}</div>
        </div>
        <div className="row-card__value">{money(s.used, s.currency)}</div>
      </div>
      {percent != null && <UsageBar value={percent} tone={tone} muted={stale} />}
    </section>
  );
}
