// Formatting helpers. Language-aware via i18n's current language.

import { getLang } from "./i18n";

export type Tone = "green" | "yellow" | "orange" | "red" | "gray";

export function toneFor(pct: number): Tone {
  if (pct < 50) return "green";
  if (pct < 75) return "yellow";
  if (pct < 90) return "orange";
  return "red";
}

/** "%20" in Turkish, "20%" in English. */
export function pct(v: number): string {
  const n = Math.round(Math.max(0, v));
  return getLang() === "tr" ? `%${n}` : `${n}%`;
}

/** "4sa 54dk" / "4h 54m"; "şimdi" / "now" when in the past. */
export function remainingTr(until: Date, now: Date): string {
  const tr = getLang() === "tr";
  const secs = Math.floor((until.getTime() - now.getTime()) / 1000);
  if (secs <= 0) return tr ? "şimdi" : "now";
  const mins = Math.ceil(secs / 60);
  const days = Math.floor(mins / 1440);
  const hours = Math.floor((mins % 1440) / 60);
  const m = mins % 60;
  const [d, h, mm] = tr ? ["g", "sa", "dk"] : ["d", "h", "m"];
  if (days > 0) return `${days}${d} ${hours}${h}`;
  if (hours > 0) return `${hours}${h} ${m}${mm}`;
  if (m > 0) return `${m}${mm}`;
  return `<1${mm}`;
}

export function isNow(remaining: string): boolean {
  return remaining === "şimdi" || remaining === "now";
}

const DAYS_TR = ["Paz", "Pzt", "Sal", "Çar", "Per", "Cum", "Cmt"];
const DAYS_EN = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

export function dayName(d: Date): string {
  return (getLang() === "tr" ? DAYS_TR : DAYS_EN)[d.getDay()];
}

/** "Pzt 14:59" / "Mon 14:59" in local time. */
export function localClockTr(d: Date): string {
  return `${dayName(d)} ${clock(d)}`;
}

export function clock(d: Date): string {
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return `${hh}:${mm}`;
}

/** "Bugün 23:10" / "Today 23:10", or "Pzt 14:59" when on another day. */
export function whenLabel(d: Date, now: Date): string {
  if (d.toDateString() === now.toDateString()) {
    return `${getLang() === "tr" ? "Bugün" : "Today"} ${clock(d)}`;
  }
  return localClockTr(d);
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

/** "az önce güncellendi" / "updated just now", "2 dk önce güncellendi" / "updated 2 min ago" */
export function agoTr(iso: string | null, now: Date): string {
  const tr = getLang() === "tr";
  if (!iso) return tr ? "henüz güncellenmedi" : "not updated yet";
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return tr ? "henüz güncellenmedi" : "not updated yet";
  const secs = Math.floor((now.getTime() - t) / 1000);
  if (secs < 45) return tr ? "az önce güncellendi" : "updated just now";
  const mins = Math.round(secs / 60);
  if (mins < 60) return tr ? `${mins} dk önce güncellendi` : `updated ${mins} min ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return tr ? `${hours} sa önce güncellendi` : `updated ${hours} h ago`;
  const d = Math.floor(hours / 24);
  return tr ? `${d} gün önce güncellendi` : `updated ${d} d ago`;
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
