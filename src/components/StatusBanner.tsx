import { AlertIcon, InfoIcon } from "./Icons";
import { t, type Key } from "../i18n";
import type { Snapshot } from "../types";

const FALLBACK: Record<Snapshot["status"], Key | null> = {
  ok: null,
  no_credentials: "bannerNoCreds",
  token_expired: "bannerTokenExpired",
  rate_limited: "bannerRateLimited",
  offline: "bannerOffline",
  error: "bannerError",
};

export default function StatusBanner({ snapshot }: { snapshot: Snapshot }) {
  if (snapshot.status === "ok") return null;
  const key = FALLBACK[snapshot.status];
  const text = snapshot.message ?? (key ? t(key) : null);
  if (!text) return null;
  const soft = snapshot.status === "token_expired" || snapshot.status === "offline";
  return (
    <div className={`banner ${soft ? "banner--soft" : "banner--warn"}`} role="status">
      {soft ? <InfoIcon /> : <AlertIcon />}
      <span>{renderInline(text)}</span>
    </div>
  );
}

/** Turn `code` spans into <code>. */
function renderInline(text: string) {
  const parts = text.split("`");
  return parts.map((p, i) => (i % 2 === 1 ? <code key={i}>{p}</code> : <span key={i}>{p}</span>));
}
