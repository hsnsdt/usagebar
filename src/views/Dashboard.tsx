import { useState, type CSSProperties } from "react";
import ContextMeter from "../components/ContextMeter";
import { GearIcon, RefreshIcon } from "../components/Icons";
import Ring from "../components/Ring";
import StatusBanner from "../components/StatusBanner";
import UsageBar from "../components/UsageBar";
import WeekCard from "../components/WeekCard";
import { agoTr, localClockTr, parseDate, remainingTr, toneFor } from "../format";
import { refreshNow } from "../hooks/useUsage";
import type { Snapshot, WindowSnap } from "../types";

const FIVE_HOURS_MS = 5 * 60 * 60 * 1000;
const SEVEN_DAYS_MS = 7 * 24 * 60 * 60 * 1000;

type Props = {
  snapshot: Snapshot;
  now: Date;
  onOpenSettings: () => void;
};

export default function Dashboard({ snapshot, now, onOpenSettings }: Props) {
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
          <button className={`iconbtn${spinning ? " spin" : ""}`} title="Yenile" aria-label="Yenile" onClick={onRefresh}>
            <RefreshIcon size={16} />
          </button>
          <button className="iconbtn" title="Ayarlar" aria-label="Ayarlar" onClick={onOpenSettings}>
            <GearIcon size={17} />
          </button>
        </div>
      </header>

      <StatusBanner snapshot={snapshot} />

      <div className="stack">
        {!apiDataHidden && (
          <>
            <Hero window={snapshot.fiveHour} now={now} stale={snapshot.stale} />
            <WeeklyRow window={snapshot.sevenDay} now={now} stale={snapshot.stale} />
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
        text = `bayat · ${text}`;
      }
      break;
    case "rate_limited":
      dot = "dot--warn";
      text = "hız sınırı, bekleniyor";
      break;
    case "offline":
      dot = "dot--warn";
      text = "çevrimdışı";
      break;
    case "token_expired":
      dot = "dot--warn";
      text = "token süresi dolmuş";
      break;
    case "no_credentials":
      dot = "dot--bad";
      text = "Claude Code bulunamadı";
      break;
    case "error":
      dot = "dot--bad";
      text = "veri alınamadı";
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
  if (util >= 90) return { text: "sınıra yakın", cls: "pace--bad" };
  if (diff > 25) return { text: "çok hızlı gidiyorsun", cls: "pace--bad" };
  if (diff > 10) return { text: "tempon yüksek", cls: "pace--warn" };
  if (diff < -25) return { text: "bol payın var", cls: "pace--ok" };
  return { text: "tempon iyi", cls: "pace--ok" };
}

function Hero({ window: w, now, stale }: { window: WindowSnap | null; now: Date; stale: boolean }) {
  if (!w) {
    return (
      <section className="hero" style={{ "--tone": "var(--gray)" } as CSSProperties}>
        <Ring size={112} stroke={11} value={0} tone="gray" muted>
          <span className="ring__pct ring__pct--dim">—</span>
        </Ring>
        <div className="hero__text">
          <div className="eyebrow">5 saatlik pencere</div>
          <div className="hero__big">veri yok</div>
        </div>
      </section>
    );
  }
  const tone = toneFor(w.utilization);
  const resetsAt = parseDate(w.resetsAt);
  const expected = pace(resetsAt, now, FIVE_HOURS_MS);
  const p = paceLabel(w.utilization, expected);
  const remaining = resetsAt ? remainingTr(resetsAt, now) : null;
  const sameDay = resetsAt ? resetsAt.toDateString() === now.toDateString() : false;
  const clock = resetsAt ? (sameDay ? localClockTr(resetsAt).slice(4) : localClockTr(resetsAt)) : null;

  return (
    <section className="hero" style={{ "--tone": `var(--${tone})` } as CSSProperties}>
      <Ring size={112} stroke={11} value={w.utilization} tone={tone} marker={expected} muted={stale}>
        <span className={`ring__pct tone-text-${tone}`}>{Math.round(w.utilization)}</span>
        <span className="ring__unit">%</span>
      </Ring>
      <div className="hero__text">
        <div className="eyebrow">5 saatlik pencere</div>
        {remaining ? (
          <>
            <div className="hero__big">
              {remaining === "şimdi" ? "sıfırlanıyor" : remaining}
            </div>
            <div className="hero__sub">
              {remaining === "şimdi" ? "yeni pencere açılıyor" : `sonra sıfırlanır · ${clock}`}
            </div>
          </>
        ) : (
          <div className="hero__sub">sıfırlanma zamanı bilinmiyor</div>
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

function WeeklyRow({ window: w, now, stale }: { window: WindowSnap | null; now: Date; stale: boolean }) {
  if (!w) {
    return (
      <section className="row-card">
        <div className="row-card__head">
          <div className="row-card__text">
            <div className="row-card__title">Haftalık</div>
            <div className="row-card__sub">veri yok</div>
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
  const sub = resetsAt ? `${localClockTr(resetsAt)}'da sıfırlanır · ${remainingTr(resetsAt, now)}` : "tüm modeller";
  return (
    <section className="row-card">
      <div className="row-card__head">
        <div className="row-card__text">
          <div className="row-card__title">Haftalık</div>
          <div className="row-card__sub">{sub}</div>
        </div>
        <div className={`row-card__value tone-text-${tone}`}>%{Math.round(w.utilization)}</div>
      </div>
      <UsageBar value={w.utilization} tone={tone} marker={expected} muted={stale} />
    </section>
  );
}
