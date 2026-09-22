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
};

export type Theme = "system" | "dark" | "light";

export type Settings = {
  pollIntervalSec: number;
  showPercentText: boolean;
  usableContextTokens: number;
  startWithWindows: boolean;
  theme: Theme;
  notifications: NotificationSettings;
};

export const MIN_POLL_INTERVAL_SEC = 180;
