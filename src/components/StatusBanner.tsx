import { AlertIcon, InfoIcon } from "./Icons";
import type { Snapshot } from "../types";

const FALLBACK: Record<Snapshot["status"], string | null> = {
  ok: null,
  no_credentials: "Claude Code bulunamadı. Terminalde claude çalıştırıp giriş yap.",
  token_expired: "Token süresi dolmuş. Terminalde bir kez claude çalıştır.",
  rate_limited: "Anthropic hız sınırı. Aşağıdaki veri bayat.",
  offline: "Bağlantı yok, son bilinen veri gösteriliyor.",
  error: "Kullanım verisi alınamadı. (detay için log)",
};

export default function StatusBanner({ snapshot }: { snapshot: Snapshot }) {
  if (snapshot.status === "ok") return null;
  const text = snapshot.message ?? FALLBACK[snapshot.status];
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
