import type { CSSProperties } from "react";
import { tokens } from "../format";
import type { DayStat } from "../types";

const DAYS_TR = ["Paz", "Pzt", "Sal", "Çar", "Per", "Cum", "Cmt"];

function dayLabel(iso: string, isToday: boolean): string {
  if (isToday) return "Bugün";
  const d = new Date(`${iso}T12:00:00`);
  return Number.isNaN(d.getTime()) ? iso.slice(5) : DAYS_TR[d.getDay()];
}

/** Percent change of `now` vs `prev`; null when there is nothing to compare. */
function delta(now: number, prev: number): number | null {
  if (prev <= 0) return now > 0 ? null : 0;
  return ((now - prev) / prev) * 100;
}

function Trend({ now, prev }: { now: number; prev: number }) {
  const d = delta(now, prev);
  if (d == null) return <span className="trend trend--flat">yeni</span>;
  if (Math.abs(d) < 1) return <span className="trend trend--flat">aynı</span>;
  const up = d > 0;
  return (
    <span className={`trend ${up ? "trend--up" : "trend--down"}`}>
      <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
        {up ? <path d="M5 1.5l3.5 4H1.5z" /> : <path d="M5 8.5l3.5-4H1.5z" />}
      </svg>
      %{Math.round(Math.abs(d))}
    </span>
  );
}

export default function WeekCard({ week }: { week: DayStat[] }) {
  if (week.length === 0) {
    return (
      <section className="row-card">
        <div className="row-card__head">
          <div className="row-card__text">
            <div className="row-card__title">Son 7 gün</div>
            <div className="row-card__sub">transcript'ler taranıyor…</div>
          </div>
        </div>
      </section>
    );
  }
  const max = Math.max(1, ...week.map((d) => d.tokens));
  const today = week[week.length - 1];
  const yesterday = week.length > 1 ? week[week.length - 2] : null;
  const weekTokens = week.reduce((a, d) => a + d.tokens, 0);

  return (
    <section className="row-card week">
      <div className="row-card__head">
        <div className="row-card__text">
          <div className="row-card__title">Son 7 gün</div>
          <div className="row-card__sub">{tokens(weekTokens)} token · düne göre bugün</div>
        </div>
      </div>

      <div className="week__chart" role="img" aria-label="Günlük token grafiği">
        {week.map((d, i) => {
          const isToday = i === week.length - 1;
          const h = Math.max(d.tokens > 0 ? 6 : 2, (d.tokens / max) * 100);
          return (
            <div className="week__col" key={d.date} title={`${d.date}: ${d.messages} mesaj, ${tokens(d.tokens)} token`}>
              <div className="week__barwrap">
                <div
                  className={`week__bar${isToday ? " week__bar--today" : ""}`}
                  style={{ "--h": `${h}%` } as CSSProperties}
                />
              </div>
              <span className={`week__day${isToday ? " week__day--today" : ""}`}>{dayLabel(d.date, isToday)}</span>
            </div>
          );
        })}
      </div>

      <div className="week__stats">
        <div className="week__stat">
          <span className="tile__k">Mesaj</span>
          <span className="week__v">
            {today.messages}
            {yesterday && <Trend now={today.messages} prev={yesterday.messages} />}
          </span>
        </div>
        <div className="week__stat">
          <span className="tile__k">Token</span>
          <span className="week__v">
            {tokens(today.tokens)}
            {yesterday && <Trend now={today.tokens} prev={yesterday.tokens} />}
          </span>
        </div>
      </div>
    </section>
  );
}
