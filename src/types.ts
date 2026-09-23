// Mirror of src-tauri/src/state.rs (serde camelCase). Keep in sync.

export type Status =
  | "ok"
  | "no_credentials"
  | "token_expired"
  | "rate_limited"
  | "offline"
  | "error";

export type WindowSnap = {
  utilization: number;
  resetsAt: string | null;
};

export type ContextSnap = {
  used: number;
  usable: number;
  project: string | null;
  model: string | null;
  /** usable was derived from the model rather than the manual setting */
  auto: boolean;
};

/** Weekly limit that only counts one model or surface (e.g. Fable). */
export type ScopedSnap = {
  label: string;
  utilization: number;
  resetsAt: string | null;
};

/** Extra usage spend, major currency units. Present only when enabled. */
export type SpendSnap = {
  used: number;
  limit: number | null;
  currency: string;
  percent: number | null;
};

/** status.claude.com; only present when the setting is on. */
export type ServiceSnap = {
  indicator: "none" | "minor" | "major" | "critical" | "maintenance" | string;
  description: string;
};

/** One chart bucket (max within the bucket); null = no data. */
export type HistoryPoint = {
  t: number;
  five: number | null;
  week: number | null;
};

export type HistoryView = {
  from: number;
  to: number;
  points: HistoryPoint[];
  peakFive: number | null;
  peakWeek: number | null;
  firstSample: number | null;
};

export type DayStat = {
  /** YYYY-MM-DD local */
  date: string;
  messages: number;
  tokens: number;
};

export type Snapshot = {
  status: Status;
  plan: string | null;
  fiveHour: WindowSnap | null;
  sevenDay: WindowSnap | null;
  scoped: ScopedSnap[];
  spend: SpendSnap | null;
  service: ServiceSnap | null;
  context: ContextSnap | null;
  today: { messages: number; tokens: number };
  /** Last 7 local days, oldest first, today last. Empty until the first scan. */
  week: DayStat[];
  lastUpdated: string | null;
  stale: boolean;
  message: string | null;
  retryInSec: number | null;
  pollIntervalSec: number;
};

export type NotificationSettings = {
  enabled: boolean;
  fiveHour: number[];
  sevenDay: number[];
  contextLowTokens: number;
  onReset: boolean;
};

export type Theme = "system" | "dark" | "light";
export type Language = "system" | "tr" | "en";
export type TimeFormat = "system" | "24h" | "12h";

export type Settings = {
  pollIntervalSec: number;
  showPercentText: boolean;
  showRemaining: boolean;
  timeFormat: TimeFormat;
  /** Global shortcut toggling the popup; "" = off. */
  hotkey: string;
  showStatus: boolean;
  miniWindow: boolean;
  /** Owned by the mini window; the UI passes it through untouched. */
  miniPos: [number, number] | null;
  usableContextTokens: number;
  autoContextWindow: boolean;
  startWithWindows: boolean;
  theme: Theme;
  language: Language;
  notifications: NotificationSettings;
};

export const MIN_POLL_INTERVAL_SEC = 180;
