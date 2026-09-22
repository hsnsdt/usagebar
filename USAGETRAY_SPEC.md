# UsageTray — Windows Claude Code Kullanım Takipçisi

> Bu dosya Claude Code'a verilecek spec'tir. Projeyi sıfırdan bu dokümana göre kur.
> Dosyayı repo köküne `SPEC.md` olarak koy, sonra Claude Code'a: **"SPEC.md'yi oku ve M1'den başla"** de.

---

## 0. Ne yapıyoruz

Windows sistem tepsisinde (system tray) yaşayan, Claude Code kullanım limitlerini gösteren bir masaüstü uygulaması.

- Tray ikonu = 5 saatlik pencerenin doluluk oranı (renk kodlu halka)
- Tıklayınca açılan şık bir popup: 5 saatlik limit, haftalık limit, reset sayaçları, kalan context, bugünkü token/mesaj sayısı
- Eşik aşıldığında Windows toast bildirimi (%50 / %75 / %90)
- Tek seferlik kurulum, arka planda çalışır, Windows'la başlar

macOS'taki [Usagebar](https://usagebar.com) uygulamasının Windows muadili. UI'ı birebir kopyalama, kendi tasarım dilimizi kuracağız.

---

## 1. Stack (karar verildi, tartışma yok)

| Katman | Seçim | Neden |
|---|---|---|
| Shell | **Tauri v2** | 5-10 MB binary, WebView2 Windows'ta zaten var, native tray + toast desteği |
| Backend | **Rust** | Credential okuma, API polling, JSONL parse, ikon render |
| Frontend | **React + TypeScript + Vite** | Popup UI'ı tamamen özelleştirilebilsin diye |
| Stil | **Plain CSS (CSS variables)** | Tailwind'e gerek yok, tek bir popup var |
| İkon render | **tiny-skia** + **ab_glyph** | Tray ikonunu runtime'da çizeceğiz |
| Bildirim | **tauri-plugin-notification** | Native Windows toast |
| Autostart | **tauri-plugin-autostart** | HKCU Run key'i kendisi yönetiyor |

Gerekli Rust crate'leri: `serde`, `serde_json`, `reqwest` (rustls-tls), `tokio`, `chrono`, `anyhow`, `thiserror`, `tiny-skia`, `ab_glyph`, `notify` (dosya izleme), `directories`, `glob`.

---

## 2. Veri kaynakları

### 2.1 Kullanım limitleri — OAuth usage endpoint

**Bu endpoint resmi değildir, dokümante edilmemiştir ve topluluk tarafından keşfedilmiştir.** Anthropic haber vermeden değiştirebilir/kapatabilir. Kod bunu varsayarak yazılmalı: şema değişirse uygulama çökmemeli, cache'e veya lokal JSONL hesabına düşmeli.

```
GET https://api.anthropic.com/api/oauth/usage

Headers:
  Authorization:  Bearer <accessToken>
  anthropic-beta: oauth-2025-04-20
  User-Agent:     claude-code/2.0.31
```

Beklenen cevap (alanlar eksik veya `null` gelebilir):

```json
{
  "five_hour": { "utilization": 33.0, "resets_at": "2026-09-22T18:00:00Z" },
  "seven_day": { "utilization": 13.0, "resets_at": "2026-09-28T14:00:00Z" }
}
```

**Kritik kurallar:**

1. **User-Agent şart.** Doğru User-Agent göndermezsen anında ve kalıcı 429 yiyorsun. Header'ı hardcode etme, `config.rs` içinde sabit tut ki tek yerden değişebilsin.
2. **Poll aralığı minimum 180 saniye**, varsayılan **300 saniye**. Ayarlardan 180'in altına indirilemesin — UI'da da engelle, backend'de de `max(180, value)` ile clamp et.
3. Her istekte 0-15 sn random jitter ekle.
4. **429 gelirse** exponential backoff: 5 dk → 10 dk → 20 dk → max 30 dk. Bu sürede cache'teki veriyi "bayat" rozetiyle göster.
5. Cevabı `%APPDATA%\UsageTray\usage-cache.json` içine yaz. Uygulama yeniden açıldığında önce cache'i göster, sonra poll et — açılışta API'yi dövme.
6. `serde` struct'larında `#[serde(default)]` kullan, bilinmeyen alanları yok say, `Option<f64>` / `Option<DateTime<Utc>>` ile çalış.

### 2.2 Credential okuma

Token'ı şu sırayla ara:

1. `CLAUDE_CODE_OAUTH_TOKEN` environment variable (varsa direkt kullan)
2. `$CLAUDE_CONFIG_DIR\.credentials.json`
3. `%USERPROFILE%\.claude\.credentials.json`  ← Windows'ta normal yol

Dosya şeması:

```json
{
  "claudeAiOauth": {
    "accessToken": "sk-ant-oat01-...",
    "refreshToken": "sk-ant-ort01-...",
    "expiresAt": 1758556800000,
    "scopes": ["user:inference", "user:profile"],
    "subscriptionType": "max"
  }
}
```

**Asla bu dosyaya yazma.** Refresh token her kullanımda rotate oluyor; biz refresh etmeye kalkarsak kullanıcının Claude Code oturumunu bozarız. Token yenilemesini Claude Code'un kendisi yapıyor.

- `expiresAt` (epoch ms) geçmişse → durum `TokenExpired`, API'yi hiç çağırma.
- Access token ~60 dakikada expire oluyor ve sadece Claude Code çalışırken yenileniyor. Kullanıcı bir süredir Claude Code açmadıysa token bayat olur — bu **normal bir durum**, hata gibi gösterme. UI'da: *"Token süresi dolmuş — terminalde bir kez `claude` çalıştır"*.
- Her poll öncesi dosyayı yeniden oku (Claude Code arada üzerine yazıyor). `notify` ile izlemek bonus.
- `subscriptionType` → popup'taki plan rozeti (Pro / Max). Yoksa rozeti gizle.

### 2.3 Lokal transcript'ler — context, token, mesaj sayısı

Konum: `%USERPROFILE%\.claude\projects\<encoded-cwd>\<session-uuid>.jsonl`

Her satır bir JSON objesi. İşimize yarayanlar `"type": "assistant"` olanlar:

```json
{
  "type": "assistant",
  "timestamp": "2026-09-22T12:40:11.123Z",
  "sessionId": "...",
  "requestId": "...",
  "message": {
    "id": "msg_...",
    "model": "claude-opus-4-...",
    "usage": {
      "input_tokens": 12,
      "output_tokens": 840,
      "cache_creation_input_tokens": 18234,
      "cache_read_input_tokens": 120480
    }
  }
}
```

**Hesaplamalar:**

- **Bugünkü toplam:** lokal gece yarısından itibaren tüm usage alanlarının toplamı. `message.id` + `requestId` ikilisiyle dedupe et (aynı mesaj birden fazla satırda görünebiliyor).
- **Bugünkü mesaj sayısı:** dedupe edilmiş assistant satırı sayısı.
- **Kalan context:** en son değiştirilen `.jsonl` dosyasındaki **son** assistant satırını al.
  `kullanılan = input_tokens + cache_read_input_tokens + cache_creation_input_tokens + output_tokens`
  `kalan = usableContextTokens - kullanılan` (varsayılan `usableContextTokens = 155_000`, ayarlardan değiştirilebilir — autocompact payını düşmüş hali)
- Bu dosyalar yüzlerce MB olabiliyor. **Tam parse etme.** Dosya başına byte offset tut, sadece yeni eklenen kısmı oku. Offset'leri `%APPDATA%\UsageTray\scan-state.json` içinde sakla, dosya küçüldüyse (truncate) offset'i sıfırla.
- `notify` ile `projects` klasörünü izle, 2 saniyelik debounce ile tara.
- Bozuk/yarım satırları sessizce atla (dosya yazılırken okuyabiliriz).

---

## 3. Mimari

```
usagetray/
├── SPEC.md                  ← bu dosya
├── CLAUDE.md                ← kısa çalışma notları (aşağıda)
├── package.json
├── vite.config.ts
├── src/                     ← frontend
│   ├── main.tsx
│   ├── App.tsx
│   ├── views/
│   │   ├── Dashboard.tsx
│   │   └── Settings.tsx
│   ├── components/
│   │   ├── UsageBar.tsx
│   │   ├── ResetTimer.tsx
│   │   ├── ContextMeter.tsx
│   │   └── StatusBanner.tsx
│   ├── hooks/useUsage.ts    ← Tauri event dinleyicisi
│   ├── types.ts
│   └── styles/
│       ├── tokens.css
│       └── app.css
└── src-tauri/
    ├── tauri.conf.json
    ├── assets/Inter-SemiBold.ttf
    └── src/
        ├── main.rs
        ├── config.rs        ← sabitler: endpoint, headers, min interval
        ├── credentials.rs   ← 2.2
        ├── usage_api.rs     ← 2.1 + backoff + cache
        ├── transcripts.rs   ← 2.3 incremental parse
        ├── tray.rs          ← dinamik ikon + menü + popup konumlama
        ├── notify.rs        ← toast + dedupe
        ├── settings.rs      ← ayar dosyası
        └── state.rs         ← AppState, poll loop, event emit
```

**Veri akışı:** Rust'ta tek bir tokio task döngüsü çalışır → state günceller → `app.emit("usage-updated", snapshot)` ile frontend'e push eder. Frontend hiç fetch yapmaz, sadece event dinler. Geri sayım sayaçları frontend'de lokal olarak tıklar (her saniye re-render), veri yenilemesi beklemez.

Ortak tip (Rust'ta serialize, TS'te aynısı `types.ts`):

```ts
type Snapshot = {
  status: 'ok' | 'no_credentials' | 'token_expired' | 'rate_limited' | 'offline' | 'error';
  plan: string | null;              // "pro" | "max" | null
  fiveHour:  { utilization: number; resetsAt: string } | null;
  sevenDay:  { utilization: number; resetsAt: string } | null;
  context:   { used: number; usable: number } | null;
  today:     { messages: number; tokens: number };
  lastUpdated: string;              // ISO
  stale: boolean;                   // cache'ten gösteriliyor
  message: string | null;           // kullanıcıya gösterilecek hata metni
};
```

---

## 4. Tray ikonu

Windows tepsi ikonu 16×16 / 32×32. macOS menu bar gibi yanına metin yazamıyoruz, **ikonu runtime'da çizeceğiz**.

- 32×32 RGBA bitmap, `tiny-skia` ile: kalınlığı 4px olan dairesel bir halka, `five_hour.utilization` oranında dolu (saat 12'den başlayıp saat yönünde). Boş kısım %25 opaklıkta.
- Renk eşikleri:
  - `< 50%` → `#4ADE80` (yeşil)
  - `50-74%` → `#FACC15` (sarı)
  - `75-89%` → `#FB923C` (turuncu)
  - `>= 90%` → `#F87171` (kırmızı)
  - hata / expired → `#94A3B8` (gri) + ortada ünlem
- Ayarlardan `showPercentText` açılırsa halkanın ortasına `ab_glyph` + gömülü Inter fontuyla sayıyı yaz (2 haneye kadar; 100 ise "99+"). Varsayılan **kapalı** — 16px'te okunmuyor.
- `tray.set_icon(...)` sadece değer değiştiğinde çağrılsın, her poll'da değil.
- **Tooltip:** `5s: %20 · 4sa 54dk sonra sıfırlanır\nHaftalık: %51`
- **Sağ tık menüsü:** Yenile / Ayarlar / Başlangıçta çalıştır (toggle) / Çıkış
- **Sol tık:** popup'ı aç/kapat

---

## 5. Popup penceresi

Tauri window ayarları: `decorations: false`, `transparent: true`, `alwaysOnTop: true`, `skipTaskbar: true`, `resizable: false`, `shadow: true`, boyut **340×460**.

- Tray ikonunun konumuna göre konumlandır: ekranın çalışma alanının (work area) sağ altına, görev çubuğunun 12px üstüne hizala. Çoklu monitörde imlecin bulunduğu monitörü baz al.
- Pencere focus kaybedince gizle (kapatma — `hide()`).
- Açılışta 120ms fade + 8px yukarı kayma animasyonu.

**Tasarım dili:**

- Koyu tema varsayılan, sistem temasını takip et (`prefers-color-scheme`).
- Kart: `#111318` zemin, `1px solid rgba(255,255,255,0.08)` kenar, `border-radius: 14px`, iç boşluk 18px.
- Font: `Segoe UI Variable Display`, fallback `Inter, system-ui`. Yüzdelerde `font-variant-numeric: tabular-nums` (sayılar zıplamasın).
- Bar'lar: 8px yüksek, tam yuvarlak uç, arkaplan `rgba(255,255,255,0.06)`, dolu kısım eşik rengiyle, `transition: width .4s ease`.
- Bölümler arası 1px ayırıcı, `rgba(255,255,255,0.06)`.
- Hiçbir yerde emoji kullanma, ikon gerekiyorsa inline SVG.

**İçerik sırası:**

1. Üst satır: `Claude Usage` + plan rozeti (`MAX`) + sağda gear ve kapat ikonları
2. **5 Saatlik Pencere** — `%20` büyük punto, altında bar, altında `4sa 54dk sonra sıfırlanır` (canlı sayaç)
3. **Haftalık** — `%51`, bar, `Pzt 14:59'da sıfırlanır`
4. Ayırıcı
5. **Context** — `88K kaldı`, bar, `155K kullanılabilirin %43'ü` + aktif oturumun proje adı
6. **Bugün** — Mesaj: `6` · Token: `753.2K` (iki sütun)
7. Alt satır: `2 dk önce güncellendi` + bayatsa turuncu nokta

**Durum banner'ları** (veri yerine tepede gösterilir):

| status | Mesaj |
|---|---|
| `no_credentials` | Claude Code bulunamadı. Terminalde `claude` çalıştırıp giriş yap. |
| `token_expired` | Token süresi dolmuş. Terminalde bir kez `claude` çalıştır. |
| `rate_limited` | Anthropic hız sınırı — X dk sonra tekrar denenecek. Aşağıdaki veri bayat. |
| `offline` | Bağlantı yok, son bilinen veri gösteriliyor. |
| `error` | Kullanım verisi alınamadı. (detay için log) |

Hata durumlarında bile lokal veriler (context, bugünkü token) gösterilmeye devam etsin — onlar API'den bağımsız.

---

## 6. Bildirimler

- 5 saatlik pencere: **%50, %75, %90**
- Haftalık: **%80, %95**
- Context: kalan **< 20K** olduğunda
- Her eşik, o pencere içinde **bir kez** tetiklenir. `resets_at` değiştiğinde dedupe bayrakları sıfırlanır. Bayraklar diske yazılsın (uygulama restart'ında tekrar bildirim atmasın).
- Metin: başlık `Claude — 5 saatlik limit %90`, gövde `4sa 12dk sonra sıfırlanıyor. İşini toparla.`
- Ayarlardan: ana açma/kapama + eşiklerin tek tek açma/kapaması.

---

## 7. Ayarlar

`%APPDATA%\UsageTray\settings.json`:

```json
{
  "pollIntervalSec": 300,
  "showPercentText": false,
  "usableContextTokens": 155000,
  "startWithWindows": true,
  "theme": "system",
  "notifications": {
    "enabled": true,
    "fiveHour": [50, 75, 90],
    "sevenDay": [80, 95],
    "contextLowTokens": 20000
  }
}
```

Ayarlar ekranı popup içinde ikinci bir view olsun, ayrı pencere açma. Dosya elle bozulmuşsa varsayılanlara dön, uygulamayı çökertme.

---

## 8. Güvenlik

- Token hiçbir log satırına yazılmaz. Log'da gösterilecekse `sk-ant-oat01-****` şeklinde maskele.
- Token sadece `api.anthropic.com` adresine gider. Başka hiçbir yere network isteği yok.
- Telemetri, analytics, crash reporting yok.
- `tauri.conf.json` CSP: `connect-src` yalnızca `https://api.anthropic.com`.
- README'de bu maddeleri açıkça yaz — insanlar token okuyan uygulamaya haklı olarak şüpheyle bakıyor.

---

## 9. Milestone'lar

Her milestone sonunda çalışan bir şey olacak. Sırayı bozma, her adımda dur ve doğrulat.

**M1 — Çekirdek (UI yok)**
`cargo run -- --probe` komutu credential'ı bulsun, API'yi çağırsın, sonucu terminale bassın. Şu PowerShell çıktısıyla birebir uyuşmalı:

```powershell
$cred  = Get-Content "$env:USERPROFILE\.claude\.credentials.json" -Raw | ConvertFrom-Json
$token = $cred.claudeAiOauth.accessToken
Invoke-RestMethod -Uri "https://api.anthropic.com/api/oauth/usage" -Headers @{
  "Authorization"  = "Bearer $token"
  "anthropic-beta" = "oauth-2025-04-20"
  "User-Agent"     = "claude-code/2.0.31"
} | ConvertTo-Json -Depth 5
```

**M2 — Tray**
Dinamik ikon, tooltip, sağ tık menüsü, poll döngüsü, cache, backoff.

**M3 — Popup UI**
Tasarım dilini kur, gerçek veriyle 5 saatlik + haftalık bölümleri, canlı sayaçlar, banner'lar.

**M4 — Lokal veriler**
JSONL incremental parser, context ölçer, bugünkü istatistikler, dosya izleme.

**M5 — Bildirim + ayarlar**
Toast'lar, dedupe, ayarlar ekranı, autostart.

**M6 — Paketleme**
`npm run tauri build` → NSIS installer. Uygulama ikonu, sürüm bilgisi, README, ilk açılışta kısa bir karşılama ekranı.

---

## 10. Kabul kriterleri

- [ ] Claude Code kurulu değilken uygulama açılıyor ve anlamlı bir mesaj gösteriyor, çökmüyor
- [ ] Token expired durumunda API'ye hiç istek gitmiyor
- [ ] Poll aralığı 180 sn'nin altına hiçbir şekilde inemiyor
- [ ] 429 sonrası backoff devreye giriyor ve cache'ten gösterim sürüyor
- [ ] Tray ikonu yüzdeye göre renk ve doluluk değiştiriyor
- [ ] Popup focus kaybedince kapanıyor, tekrar tıklayınca açılıyor
- [ ] Geri sayım sayaçları API beklemeden her saniye güncelleniyor
- [ ] 500 MB'lık bir `projects` klasöründe ilk tarama 3 sn'nin altında, sonraki taramalar anlık
- [ ] Her eşik için pencere başına tek bildirim gidiyor
- [ ] Uygulama restart'ında eşik bildirimleri tekrarlanmıyor
- [ ] Log'ların hiçbir yerinde token görünmüyor
- [ ] Installer temiz bir Windows 11 makinesinde çalışıyor
- [ ] Boşta RAM < 80 MB, CPU ~%0

---

## 11. Tuzaklar (Claude Code buraya dikkat)

1. Poll aralığını test ederken düşürme. 429 yersen access token bazında yersin ve dakikalarca beklersin.
2. `User-Agent` header'ını unutma veya değiştirme.
3. `.credentials.json` dosyasına **yazma**.
4. `resets_at` UTC geliyor, kullanıcıya lokal saat dilimiyle göster.
5. JSONL dosyalarını her seferinde baştan okuma.
6. Tauri v2 API'si v1'den farklı — `tauri::tray::TrayIconBuilder`, `app.emit`, plugin isimleri değişti. v1 örneklerini kopyalama, [v2 dokümanına](https://v2.tauri.app) bak.
7. Windows 11'de tepsi ikonu varsayılan olarak taşma menüsünde gizlenebiliyor. README'de "ikonu görev çubuğuna sürükle" notu olsun.
8. WebView2 runtime Win11'de var, Win10'da olmayabilir — installer'a bootstrapper ekle.
9. Endpoint dokümante değil. Şema beklenmedik gelirse `status: 'error'` ver, `unwrap()` ile paniğe girme.
10. Emin olmadığın bir API veya alan adı uydurma. Bilmiyorsan bana sor.

---

## 12. CLAUDE.md içeriği (repo köküne ayrıca koy)

```md
# UsageTray

Windows tray app, Claude Code kullanım limitlerini gösterir. Tam spec: SPEC.md

## Kurallar
- Tauri v2 + React + TS + Rust. v1 örneği kopyalama.
- `.credentials.json` READ-ONLY. Asla yazma, asla refresh etme.
- usage API poll aralığı minimum 180 sn. Test ederken de.
- User-Agent header'ı zorunlu: claude-code/2.0.31
- Token loglama. Maskele.
- Değişiklikten sonra `npm run tauri dev` ile çalıştığını doğrula.

## Komutlar
npm run tauri dev      # geliştirme
npm run tauri build    # installer üret
cargo test             # rust testleri
cargo run -- --probe   # credential + API doğrulama
```
