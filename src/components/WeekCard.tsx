import type { CSSProperties } from "react";
import { dayName, pct, tokens } from "../format";
import { t } from "../i18n";
import type { DayStat } from "../types";

function dayLabel(iso: string, isToday: boolean): string {
  if (isToday) return t("today");
  const d = new Date(`${iso}T12:00:00`);
  return Number.isNaN(d.getTime()) ? iso.slice(5) : dayName(d);
}

/** Percent change of `now` vs `prev`; null when there is nothing to compare. */
function delta(now: number, prev: number): number | null {
  if (prev <= 0) return now > 0 ? null : 0;
  return ((now - prev) / prev) * 100;
}

function Trend({ now, prev }: { now: number; prev: number }) {
  const d = delta(now, prev);
  if (d == null) return <span className="trend trend--flat">{t("trendNew")}</span>;
  if (Math.abs(d) < 1) return <span className="trend trend--flat">{t("trendSame")}</span>;
  const up = d > 0;
  return (
    <span className={`trend ${up ? "trend--up" : "trend--down"}`}>
      <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
        {up ? <path d="M5 1.5l3.5 4H1.5z" /> : <path d="M5 8.5l3.5-4H1.5z" />}
      </svg>
      {pct(Math.abs(d))}
    </span>
  );
}

export default function WeekCard({ week }: { week: DayStat[] }) {
  if (week.length === 0) {
    return (
      <section className="row-card">
        <div className="row-card__head">
          <div className="row-card__text">
            <div className="row-card__title">{t("last7")}</div>
            <div className="row-card__sub">{t("scanning")}</div>
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
          <div className="row-card__title">{t("last7")}</div>
          <div className="row-card__sub">{t("weekSub", { tokens: tokens(weekTokens) })}</div>
        </div>
      </div>

      <div className="week__chart" role="img" aria-label={t("chartAria")}>
        {week.map((d, i) => {
          const isToday = i === week.length - 1;
          const h = Math.max(d.tokens > 0 ? 6 : 2, (d.tokens / max) * 100);
          return (
            <div
              className="week__col"
              key={d.date}
              title={t("dayTitle", { date: d.date, messages: d.messages, tokens: tokens(d.tokens) })}
            >
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
          <span className="tile__k">{t("messages")}</span>
          <span className="week__v">
            {today.messages}
            {yesterday && <Trend now={today.messages} prev={yesterday.messages} />}
          </span>
        </div>
        <div className="week__stat">
          <span className="tile__k">{t("tokens")}</span>
          <span className="week__v">
            {tokens(today.tokens)}
            {yesterday && <Trend now={today.tokens} prev={yesterday.tokens} />}
          </span>
        </div>
      </div>
    </section>
  );
}
