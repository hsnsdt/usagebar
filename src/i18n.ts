// Tiny i18n: a module-level language plus a subscribe hook. Rust keeps its
// own copy of the same rules in src-tauri/src/i18n.rs (tray, toasts).

import { useSyncExternalStore } from "react";

export type Lang = "tr" | "en";
export type LangSetting = "system" | Lang;

const STR = {
  tr: {
    // header / status
    refresh: "Yenile",
    settings: "Ayarlar",
    back: "Geri",
    close: "Kapat",
    save: "Kaydet",
    saved: "Kaydedildi",
    loading: "Yükleniyor…",
    statusStale: "bayat",
    statusRateLimited: "hız sınırı, bekleniyor",
    statusOffline: "çevrimdışı",
    statusTokenExpired: "token süresi dolmuş",
    statusNoCreds: "Claude Code bulunamadı",
    statusError: "veri alınamadı",
    // hero
    fiveHourWindow: "5 saatlik pencere",
    noData: "veri yok",
    resetting: "sıfırlanıyor",
    newWindow: "yeni pencere açılıyor",
    untilReset: "sonra sıfırlanır · {clock}",
    resetUnknown: "sıfırlanma zamanı bilinmiyor",
    paceNearLimit: "sınıra yakın",
    paceBurning: "çok hızlı gidiyorsun",
    paceHigh: "tempon yüksek",
    paceHeadroom: "bol payın var",
    paceFine: "tempon iyi",
    // weekly
    weekly: "Haftalık",
    weeklyResets: "{clock}'da sıfırlanır · {remaining}",
    allModels: "tüm modeller",
    // context
    context: "Context",
    noSession: "Aktif oturum yok",
    activeSession: "Aktif oturum",
    left: "kaldı",
    contextFoot: "{usable} kullanılabilir · {pct} dolu",
    // week card
    last7: "Son 7 gün",
    scanning: "transcript'ler taranıyor…",
    weekSub: "{tokens} token · düne göre bugün",
    today: "Bugün",
    messages: "Mesaj",
    tokens: "Token",
    trendNew: "yeni",
    trendSame: "aynı",
    chartAria: "Günlük token grafiği",
    dayTitle: "{date}: {messages} mesaj, {tokens} token",
    // banners (fallbacks; Rust normally supplies the text)
    bannerNoCreds: "Claude Code bulunamadı. Terminalde `claude` çalıştırıp giriş yap.",
    bannerTokenExpired: "Token süresi dolmuş. Terminalde bir kez `claude` çalıştır.",
    bannerRateLimited: "Anthropic hız sınırı. Aşağıdaki veri bayat.",
    bannerOffline: "Bağlantı yok, son bilinen veri gösteriliyor.",
    bannerError: "Kullanım verisi alınamadı. (detay için log)",
    // settings
    sGeneral: "Genel",
    sPoll: "Yenileme aralığı",
    sPollHint: "en az {min} sn",
    sSec: "sn",
    sContext: "Kullanılabilir context",
    sContextHint: "autocompact payı düşülmüş",
    sToken: "token",
    sTheme: "Tema",
    themeSystem: "Sistem",
    themeDark: "Koyu",
    themeLight: "Açık",
    sLanguage: "Dil",
    langSystem: "Sistem",
    sTray: "Tepsi",
    sPercentText: "İkonda yüzde yazısı",
    sPercentHint: "16px'te zor okunur",
    sAutostart: "Windows ile başlat",
    sAutostartHint: "arka planda, görev çubuğu olmadan",
    sNotifications: "Bildirimler",
    sNotifEnabled: "Bildirimler açık",
    sFiveThresholds: "5 saatlik eşikler",
    sWeekThresholds: "Haftalık eşikler",
    sContextLow: "Context uyarısı",
    sContextLowHint: "kalan token bunun altına inince",
    sOpenLogs: "Log klasörünü aç",
    sQuit: "Çıkış",
    // welcome
    wTitle: "UsageTray'e hoş geldin",
    wLead: "Claude Code limitlerin artık tepside. Halka 5 saatlik pencerenin doluluğunu gösterir; tıklayınca bu panel açılır.",
    wTrayTitle: "Tepsi ikonu",
    wTrayBody: "Windows 11 ikonu taşma menüsüne gizleyebilir; görünür olsun istersen görev çubuğuna sürükle.",
    wPrivacyTitle: "Gizlilik",
    wPrivacyBody: "Token yalnızca api.anthropic.com adresine gider. Credential dosyası hiç yazılmaz, telemetri yoktur.",
    wNotifyTitle: "Bildirimler",
    wNotifyBody: "%50, %75 ve %90'da birer kez uyarı alırsın; eşikler ayarlardan değişir.",
    wStart: "Başla",
    // about
    about: "Hakkında",
    aboutTagline: "Claude Code kullanım takipçisi",
    aboutDeveloper: "Geliştirici",
    aboutWebsite: "Web sitesi",
    aboutProject: "Proje",
    aboutSource: "Kaynak kod",
    aboutIssue: "Hata bildir / öneri",
    aboutIssueSub: "GitHub Issues",
    aboutReleases: "Sürümler",
    aboutReleasesSub: "Yeni sürüm var mı diye bak",
    aboutLicense: "Lisans",
    aboutPrivacy: "Token yalnızca api.anthropic.com adresine gider. Telemetri yok, credential dosyası hiç yazılmaz.",
  },
  en: {
    refresh: "Refresh",
    settings: "Settings",
    back: "Back",
    close: "Close",
    save: "Save",
    saved: "Saved",
    loading: "Loading…",
    statusStale: "stale",
    statusRateLimited: "rate limited, waiting",
    statusOffline: "offline",
    statusTokenExpired: "token expired",
    statusNoCreds: "Claude Code not found",
    statusError: "couldn't fetch usage",
    fiveHourWindow: "5-hour window",
    noData: "no data",
    resetting: "resetting",
    newWindow: "new window starting",
    untilReset: "until reset · {clock}",
    resetUnknown: "reset time unknown",
    paceNearLimit: "near the limit",
    paceBurning: "burning fast",
    paceHigh: "pace is high",
    paceHeadroom: "plenty of headroom",
    paceFine: "pace is fine",
    weekly: "Weekly",
    weeklyResets: "resets {clock} · {remaining}",
    allModels: "all models",
    context: "Context",
    noSession: "No active session",
    activeSession: "Active session",
    left: "left",
    contextFoot: "{usable} usable · {pct} used",
    last7: "Last 7 days",
    scanning: "scanning transcripts…",
    weekSub: "{tokens} tokens · today vs yesterday",
    today: "Today",
    messages: "Messages",
    tokens: "Tokens",
    trendNew: "new",
    trendSame: "same",
    chartAria: "Daily token chart",
    dayTitle: "{date}: {messages} messages, {tokens} tokens",
    bannerNoCreds: "Claude Code not found. Run `claude` in a terminal and sign in.",
    bannerTokenExpired: "Token expired. Run `claude` once in a terminal.",
    bannerRateLimited: "Anthropic rate limit. Data below is stale.",
    bannerOffline: "No connection, showing last known data.",
    bannerError: "Couldn't fetch usage data. (see logs)",
    sGeneral: "General",
    sPoll: "Refresh interval",
    sPollHint: "at least {min} s",
    sSec: "s",
    sContext: "Usable context",
    sContextHint: "autocompact share excluded",
    sToken: "tokens",
    sTheme: "Theme",
    themeSystem: "System",
    themeDark: "Dark",
    themeLight: "Light",
    sLanguage: "Language",
    langSystem: "System",
    sTray: "Tray",
    sPercentText: "Percent text on icon",
    sPercentHint: "hard to read at 16px",
    sAutostart: "Start with Windows",
    sAutostartHint: "in the background, no taskbar entry",
    sNotifications: "Notifications",
    sNotifEnabled: "Notifications on",
    sFiveThresholds: "5-hour thresholds",
    sWeekThresholds: "Weekly thresholds",
    sContextLow: "Context warning",
    sContextLowHint: "when remaining tokens drop below",
    sOpenLogs: "Open log folder",
    sQuit: "Quit",
    wTitle: "Welcome to UsageTray",
    wLead: "Your Claude Code limits now live in the tray. The ring shows the 5-hour window; click it to open this panel.",
    wTrayTitle: "Tray icon",
    wTrayBody: "Windows 11 may hide it in the overflow menu; drag it onto the taskbar to keep it visible.",
    wPrivacyTitle: "Privacy",
    wPrivacyBody: "The token only ever goes to api.anthropic.com. The credential file is never written, there is no telemetry.",
    wNotifyTitle: "Notifications",
    wNotifyBody: "One alert each at 50%, 75% and 90%; thresholds are adjustable in settings.",
    wStart: "Get started",
    about: "About",
    aboutTagline: "Claude Code usage tracker",
    aboutDeveloper: "Developer",
    aboutWebsite: "Website",
    aboutProject: "Project",
    aboutSource: "Source code",
    aboutIssue: "Report a bug / idea",
    aboutIssueSub: "GitHub Issues",
    aboutReleases: "Releases",
    aboutReleasesSub: "Check for a newer version",
    aboutLicense: "License",
    aboutPrivacy: "The token only goes to api.anthropic.com. No telemetry; the credential file is never written.",
  },
} as const;

export type Key = keyof (typeof STR)["tr"];

let current: Lang = detect("system");
const listeners = new Set<() => void>();

export function detect(setting: LangSetting): Lang {
  if (setting === "tr" || setting === "en") return setting;
  const nav = (navigator.language || "en").toLowerCase();
  return nav.startsWith("tr") ? "tr" : "en";
}

export function setLanguage(setting: LangSetting) {
  const next = detect(setting);
  if (next === current) return;
  current = next;
  document.documentElement.lang = next;
  listeners.forEach((l) => l());
}

export function getLang(): Lang {
  return current;
}

export function useLang(): Lang {
  return useSyncExternalStore(
    (cb) => {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    () => current,
  );
}

export function t(key: Key, params?: Record<string, string | number>): string {
  let s: string = STR[current][key] ?? STR.en[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) s = s.replaceAll(`{${k}}`, String(v));
  }
  return s;
}
