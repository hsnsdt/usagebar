// Formatting helpers. Turkish copy, tabular numbers handled by CSS.

export type Tone = "green" | "yellow" | "orange" | "red" | "gray";

export function toneFor(pct: number): Tone {
  if (pct < 50) return "green";
  if (pct < 75) return "yellow";
  if (pct < 90) return "orange";
  return "red";
}

export function pct(v: number): string {
  return `%${Math.round(Math.max(0, v))}`;
}

/** "4sa 54dk" / "12dk" / "şimdi" */
export function remainingTr(until: Date, now: Date): string {
  const secs = Math.floor((until.getTime() - now.getTime()) / 1000);
  if (secs <= 0) return "şimdi";
  const mins = Math.ceil(secs / 60);
  const days = Math.floor(mins / 1440);
  const hours = Math.floor((mins % 1440) / 60);
  const m = mins % 60;
  if (days > 0) return `${days}g ${hours}sa`;
  if (hours > 0) return `${hours}sa ${m}dk`;
  if (m > 0) return `${m}dk`;
  return "<1dk";
}

const DAYS_TR = ["Paz", "Pzt", "Sal", "Çar", "Per", "Cum", "Cmt"];

/** "Pzt 14:59" in local time */
export function localClockTr(d: Date): string {
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return `${DAYS_TR[d.getDay()]} ${hh}:${mm}`;
}

/** 753200 -> "753.2K", 1234567 -> "1.23M", 880 -> "880" */
export function tokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n >= 10_000_000 ? 1 : 2)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(n >= 100_000 ? 0 : 1)}K`;
  return String(n);
}

/** "88K" style for context figures (no decimals) */
export function tokensK(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  return `${Math.round(n / 1000)}K`;
}

/** "az önce" / "2 dk önce" / "3 sa önce" */
export function agoTr(iso: string | null, now: Date): string {
  if (!iso) return "henüz güncellenmedi";
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return "henüz güncellenmedi";
  const secs = Math.floor((now.getTime() - t) / 1000);
  if (secs < 45) return "az önce güncellendi";
  const mins = Math.round(secs / 60);
  if (mins < 60) return `${mins} dk önce güncellendi`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours} sa önce güncellendi`;
  return `${Math.floor(hours / 24)} gün önce güncellendi`;
}

export function parseDate(iso: string | null): Date | null {
  if (!iso) return null;
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? null : d;
}

/** Last path component of an encoded project dir, prettified. */
export function projectLabel(p: string | null): string | null {
  if (!p) return null;
  return p.length > 28 ? `…${p.slice(-27)}` : p;
}
