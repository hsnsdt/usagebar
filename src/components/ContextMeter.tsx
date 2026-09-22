import UsageBar from "./UsageBar";
import { pct, projectLabel, toneFor, tokensK } from "../format";
import { t } from "../i18n";
import type { ContextSnap } from "../types";

function modelShort(m: string | null): string | null {
  if (!m) return null;
  // "claude-fable-5-1" -> "Fable 5.1", "claude-opus-4-1-20250805" -> "Opus 4.1"
  const parts = m.replace(/^claude-/, "").split("-");
  const name = parts[0];
  const nums = parts.slice(1).filter((p) => /^\d+$/.test(p) && p.length < 4);
  const ver = nums.slice(0, 2).join(".");
  return `${name.charAt(0).toUpperCase()}${name.slice(1)}${ver ? ` ${ver}` : ""}`;
}

export default function ContextMeter({ context }: { context: ContextSnap | null }) {
  if (!context) {
    return (
      <section className="row-card">
        <div className="row-card__head">
          <div className="row-card__text">
            <div className="row-card__title">{t("context")}</div>
            <div className="row-card__sub">{t("noSession")}</div>
          </div>
          <div className="row-card__value row-card__value--dim">—</div>
        </div>
        <UsageBar value={0} tone="gray" muted />
      </section>
    );
  }
  const used = Math.min(context.used, context.usable);
  const remaining = Math.max(0, context.usable - context.used);
  const usedPct = context.usable > 0 ? (used / context.usable) * 100 : 0;
  const tone = toneFor(usedPct);
  const project = projectLabel(context.project);
  const model = modelShort(context.model);
  const sub = [project, model].filter(Boolean).join(" · ") || t("activeSession");
  return (
    <section className="row-card">
      <div className="row-card__head">
        <div className="row-card__text">
          <div className="row-card__title">{t("context")}</div>
          <div className="row-card__sub" title={context.project ?? undefined}>
            {sub}
          </div>
        </div>
        <div className="row-card__value">
          <span className={`tone-text-${tone}`}>{tokensK(remaining)}</span>
          <span className="row-card__unit"> {t("left")}</span>
        </div>
      </div>
      <UsageBar value={usedPct} tone={tone} />
      <div className="row-card__foot">{t("contextFoot", { usable: tokensK(context.usable), pct: pct(usedPct) })}</div>
    </section>
  );
}
