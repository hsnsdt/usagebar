import type { CSSProperties } from "react";
import type { Tone } from "../format";

type Props = {
  /** 0..100 */
  value: number;
  tone: Tone;
  /** 0..100: where usage "should" be at this point of the window. */
  marker?: number | null;
  /** Dim the bar (stale data). */
  muted?: boolean;
};

export default function UsageBar({ value, tone, marker, muted }: Props) {
  const w = Math.max(0, Math.min(100, value));
  const m = marker == null ? null : Math.max(0, Math.min(100, marker));
  const style = { "--tone": `var(--${tone})` } as CSSProperties;
  return (
    <div
      className={`bar${muted ? " bar--muted" : ""}`}
      style={style}
      role="progressbar"
      aria-valuenow={Math.round(w)}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div className="bar__fill" style={{ width: `${w}%` }} />
      {m != null && m > 1 && m < 99 && <div className="bar__marker" style={{ left: `${m}%` }} />}
    </div>
  );
}
