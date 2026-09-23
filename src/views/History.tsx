import { useEffect, useMemo, useState, type MouseEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { BackIcon } from "../components/Icons";
import { clock, dayName, pct } from "../format";
import { t, useLang } from "../i18n";
import type { HistoryPoint, HistoryView } from "../types";

const RANGES = [
  { hours: 24, key: "hRange24" },
  { hours: 24 * 7, key: "hRange7" },
  { hours: 24 * 30, key: "hRange30" },
] as const;

/** Unchanged values are recorded every 30 min; a longer silence is a gap. */
const MAX_JOIN_MS = 40 * 60 * 1000;

// Chart geometry (px). Width matches the card's inner width.
const W = 284;
const H = 150;
const PAD = { l: 26, r: 30, t: 8, b: 18 };

type SeriesKey = "five" | "week";
const SERIES: { key: SeriesKey; label: "fiveHourShort" | "weekly"; cls: string }[] = [
  { key: "five", label: "fiveHourShort", cls: "s1" },
  { key: "week", label: "weekly", cls: "s2" },
];

export default function History({ onBack }: { onBack: () => void }) {
  useLang();
  const [hours, setHours] = useState<number>(24);
  const [view, setView] = useState<HistoryView | null>(null);
  const [exportMsg, setExportMsg] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    invoke<HistoryView>("get_history", { hours })
      .then((v) => alive && setView(v))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [hours]);

  const onExport = async () => {
    try {
      await invoke<string>("export_history_csv");
      setExportMsg(t("hExported"));
    } catch {
      setExportMsg(t("hExportEmpty"));
    }
    window.setTimeout(() => setExportMsg(null), 2500);
  };

  const hasData = !!view && view.points.some((p) => p.five != null || p.week != null);

  return (
    <>
      <header className="head head--sub">
        <div className="head__text">
          <div className="head__title">
            <button className="iconbtn iconbtn--back" title={t("back")} aria-label={t("back")} onClick={onBack}>
              <BackIcon />
            </button>
            <span>{t("history")}</span>
          </div>
        </div>
      </header>

      <div className="stack">
        <div className="chips chips--range" role="tablist">
          {RANGES.map((r) => (
            <button
              key={r.hours}
              role="tab"
              aria-selected={hours === r.hours}
              className={`chip${hours === r.hours ? " chip--on" : ""}`}
              onClick={() => setHours(r.hours)}
            >
              {t(r.key)}
            </button>
          ))}
        </div>

        <section className="row-card">
          <div className="legend">
            {SERIES.map((s) => (
              <span key={s.key} className="legend__item">
                <span className={`legend__swatch ${s.cls}`} />
                {t(s.label)}
              </span>
            ))}
          </div>
          {view && hasData ? (
            <Chart view={view} hours={hours} />
          ) : (
            <div className="hchart__empty">{view ? t("hEmpty") : t("loading")}</div>
          )}
        </section>

        {view && hasData && (
          <div className="today">
            <div className="tile">
              <span className="tile__k">{t("hPeakFive")}</span>
              <span className="tile__v">{view.peakFive != null ? pct(view.peakFive) : "—"}</span>
            </div>
            <div className="tile">
              <span className="tile__k">{t("hPeakWeek")}</span>
              <span className="tile__v">{view.peakWeek != null ? pct(view.peakWeek) : "—"}</span>
            </div>
          </div>
        )}

        <div className="hfoot">
          <span className="hfoot__note">
            {view?.firstSample ? t("hSince", { date: dateLabel(new Date(view.firstSample)) }) : t("hLocal")}
          </span>
          <button className="btn" onClick={onExport}>
            {exportMsg ?? t("hExport")}
          </button>
        </div>
      </div>
    </>
  );
}

function dateLabel(d: Date): string {
  return `${d.getDate()}.${String(d.getMonth() + 1).padStart(2, "0")}`;
}

function tickLabel(ms: number, hours: number): string {
  const d = new Date(ms);
  return hours <= 24 ? clock(d) : `${dayName(d)} ${d.getDate()}`;
}

type Seg = { x: number; y: number; t: number; v: number }[];

function Chart({ view, hours }: { view: HistoryView; hours: number }) {
  const [hover, setHover] = useState<number | null>(null);
  const span = Math.max(1, view.to - view.from);
  const bucket = view.points.length > 1 ? view.points[1].t - view.points[0].t : span;
  const x = (ms: number) => PAD.l + ((ms - view.from) / span) * (W - PAD.l - PAD.r);
  const y = (v: number) => PAD.t + (1 - Math.max(0, Math.min(100, v)) / 100) * (H - PAD.t - PAD.b);

  // Segments per series; a silence longer than MAX_JOIN_MS breaks the line.
  const segments = useMemo(() => {
    const out: Record<SeriesKey, Seg[]> = { five: [], week: [] };
    for (const s of SERIES) {
      let cur: Seg = [];
      let lastT = -Infinity;
      for (const p of view.points) {
        const v = p[s.key];
        if (v == null) continue;
        const tMid = p.t + bucket / 2;
        if (tMid - lastT > MAX_JOIN_MS + bucket && cur.length) {
          out[s.key].push(cur);
          cur = [];
        }
        cur.push({ x: x(tMid), y: y(v), t: tMid, v });
        lastT = tMid;
      }
      if (cur.length) out[s.key].push(cur);
    }
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view]);

  const path = (seg: Seg) =>
    seg.length === 1
      ? `M${seg[0].x - 1.5},${seg[0].y}h3`
      : seg.map((p, i) => `${i ? "L" : "M"}${p.x.toFixed(1)},${p.y.toFixed(1)}`).join("");

  // End-of-line value labels, nudged apart when they would collide.
  const ends = SERIES.map((s) => {
    const segs = segments[s.key];
    const last = segs.length ? segs[segs.length - 1][segs[segs.length - 1].length - 1] : null;
    return last ? { key: s.key, cls: s.cls, x: last.x, y: last.y, v: last.v } : null;
  }).filter((e): e is NonNullable<typeof e> => e != null);
  if (ends.length === 2 && Math.abs(ends[0].y - ends[1].y) < 11) {
    const [hi, lo] = ends[0].y <= ends[1].y ? [ends[0], ends[1]] : [ends[1], ends[0]];
    const mid = (hi.y + lo.y) / 2;
    hi.y = mid - 5.5;
    lo.y = mid + 5.5;
  }

  const onMove = (e: MouseEvent<SVGSVGElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const px = ((e.clientX - r.left) / r.width) * W;
    const ms = view.from + ((px - PAD.l) / (W - PAD.l - PAD.r)) * span;
    let best: number | null = null;
    let bestD = Infinity;
    view.points.forEach((p, i) => {
      if (p.five == null && p.week == null) return;
      const d = Math.abs(p.t + bucket / 2 - ms);
      if (d < bestD) {
        bestD = d;
        best = i;
      }
    });
    setHover(best);
  };

  const hp: HistoryPoint | null = hover != null ? view.points[hover] : null;
  const hx = hp ? x(hp.t + bucket / 2) : 0;
  const ticks = [view.from, view.from + span / 2, view.to];
  const summary = t("hAria", {
    five: view.peakFive != null ? pct(view.peakFive) : "—",
    week: view.peakWeek != null ? pct(view.peakWeek) : "—",
  });

  return (
    <div className="hchart">
      <svg
        width={W}
        height={H}
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={summary}
        onMouseMove={onMove}
        onMouseLeave={() => setHover(null)}
      >
        {[0, 50, 100].map((g) => (
          <g key={g}>
            <line className="hchart__grid" x1={PAD.l} x2={W - PAD.r} y1={y(g)} y2={y(g)} />
            <text className="hchart__axis" x={PAD.l - 6} y={y(g) + 3.5} textAnchor="end">
              {g}
            </text>
          </g>
        ))}
        {ticks.map((ms, i) => (
          <text
            key={i}
            className="hchart__axis"
            x={x(ms)}
            y={H - 4}
            textAnchor={i === 0 ? "start" : i === 2 ? "end" : "middle"}
          >
            {tickLabel(ms, hours)}
          </text>
        ))}
        {SERIES.map((s) =>
          segments[s.key].map((seg, i) => <path key={`${s.key}${i}`} className={`hchart__line ${s.cls}`} d={path(seg)} />),
        )}
        {ends.map((e) => (
          <g key={e.key}>
            <circle className={`hchart__dot ${e.cls}`} cx={e.x} cy={segments[e.key].at(-1)!.at(-1)!.y} r={3} />
            <text className="hchart__end" x={e.x + 6} y={e.y + 3.5}>
              {Math.round(e.v)}
            </text>
          </g>
        ))}
        {hp && (
          <g>
            <line className="hchart__cross" x1={hx} x2={hx} y1={PAD.t} y2={H - PAD.b} />
            {SERIES.map((s) =>
              hp[s.key] != null ? (
                <circle key={s.key} className={`hchart__dot ${s.cls}`} cx={hx} cy={y(hp[s.key]!)} r={3.5} />
              ) : null,
            )}
          </g>
        )}
      </svg>
      {hp && (
        <div className={`hchart__tip${hx > W / 2 ? " hchart__tip--left" : ""}`} style={{ left: hx }}>
          <div className="hchart__tip-t">
            {hours <= 24 ? clock(new Date(hp.t)) : `${dayName(new Date(hp.t))} ${dateLabel(new Date(hp.t))} ${clock(new Date(hp.t))}`}
          </div>
          {SERIES.map((s) => (
            <div key={s.key} className="hchart__tip-row">
              <span className={`legend__swatch ${s.cls}`} />
              <span>{t(s.label)}</span>
              <b>{hp[s.key] != null ? pct(hp[s.key]!) : "—"}</b>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
