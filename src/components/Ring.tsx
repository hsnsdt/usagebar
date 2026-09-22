import type { CSSProperties, ReactNode } from "react";
import type { Tone } from "../format";

type Props = {
  size: number;
  stroke: number;
  /** 0..100 */
  value: number;
  tone: Tone;
  /** 0..100 expected pace; drawn as a tick on the track. */
  marker?: number | null;
  muted?: boolean;
  children?: ReactNode;
};

/** Circular gauge that mirrors the tray icon: 12 o'clock start, clockwise. */
export default function Ring({ size, stroke, value, tone, marker, muted, children }: Props) {
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const v = Math.max(0, Math.min(100, value)) / 100;
  const offset = c * (1 - v);
  const style = { "--tone": `var(--${tone})` } as CSSProperties;

  let tick: ReactNode = null;
  if (marker != null && marker > 1 && marker < 99) {
    const a = (marker / 100) * 2 * Math.PI - Math.PI / 2;
    const cx = size / 2 + r * Math.cos(a);
    const cy = size / 2 + r * Math.sin(a);
    tick = <circle className="ring__tick" cx={cx} cy={cy} r={stroke * 0.28} />;
  }

  return (
    <div className={`ring${muted ? " ring--muted" : ""}`} style={{ width: size, height: size, ...style }}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-hidden="true">
        <circle className="ring__track" cx={size / 2} cy={size / 2} r={r} strokeWidth={stroke} />
        <circle
          className="ring__arc"
          cx={size / 2}
          cy={size / 2}
          r={r}
          strokeWidth={stroke}
          strokeDasharray={c}
          strokeDashoffset={offset}
          transform={`rotate(-90 ${size / 2} ${size / 2})`}
        />
        {tick}
      </svg>
      <div className="ring__center">
        <span className="ring__num">{children}</span>
      </div>
    </div>
  );
}
