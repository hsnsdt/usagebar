# UsageTray

Windows tray app, Claude Code kullanım limitlerini gösterir. Tam spec: USAGETRAY_SPEC.md

## Kurallar
- Tauri v2 + React + TS + Rust. v1 örneği kopyalama.
- `.credentials.json` READ-ONLY. Asla yazma, asla refresh etme.
- usage API poll aralığı minimum 180 sn. Test ederken de: her debug açılışı gerçek API'ye istek atar, iki açılış arası ≥180 sn.
- User-Agent header'ı zorunlu: claude-code/2.0.31 (tek kaynak: `src-tauri/src/config.rs`)
- Token loglama. Maskele (`credentials::mask`).
- Telemetri yok. Token sadece api.anthropic.com'a gider. Tek başka host status.claude.com, o da ayarla açılır (varsayılan kapalı, token gitmez). Yeni host ekleme = README güvenlik bölümünü de güncelle.
- Pencere oluşturmayı (`WebviewWindowBuilder::build`) sync komut veya menü/tray handler içinde yapma: Windows'ta event loop kilitlenir (wry#583). Ayrı thread'de oluştur (`mini.rs` gibi).
- Değişiklikten sonra `npm run tauri dev` ile çalıştığını doğrula. Popup'a tıklayarak test ederken `--pin` kullan.
- Rust ve TS metinleri iki yerde: `src-tauri/src/i18n.rs` (tray, tooltip, toast) ve `src/i18n.ts` (UI). TR + EN birlikte güncellenir.
- Kullanıcı mevcut koyu temayı istiyor; büyük yeniden tasarım yapma, küçük dokunuşlar.

## Komutlar
npm run tauri dev      # geliştirme
npm run tauri build    # installer üret
cargo test             # rust testleri
cargo run -- --probe   # credential + API doğrulama (ham API cevabını basar)
usagetray.exe --show / --settings / --about / --pin   # debug bayrakları

## Release
Versiyonu `src-tauri/tauri.conf.json`, `package.json`, `src-tauri/Cargo.toml` içinde yükselt → `npm install --package-lock-only` → commit → `git tag -a vX.Y.Z` + push. `.github/workflows/release.yml` installer + portable + SHA256SUMS üretir; tag, tauri.conf.json versiyonuyla aynı olmalı.

## Yerleşim
- `src-tauri/src/config.rs`      sabitler (endpoint, header, limitler, dosya adları), atomik yazma
- `src-tauri/src/credentials.rs` credential okuma (salt okunur)
- `src-tauri/src/usage_api.rs`   API istemcisi, cache, backoff; `limits[]` (modele özel limitler), `spend` (ekstra kullanım)
- `src-tauri/src/state.rs`       AppState, Snapshot, poll döngüsü (uykudan uyanma algılama), `usage-updated` event
- `src-tauri/src/settings.rs`    settings.json, `sanitized()` ile sınırlar
- `src-tauri/src/transcripts.rs` JSONL artımlı tarama (context, günlük token)
- `src-tauri/src/context.rs`     modele göre context penceresi
- `src-tauri/src/tray.rs`        dinamik ikon, tooltip, menü, popup konumu
- `src-tauri/src/toasts.rs`      toast + dedupe (eşikler, reset, model limitleri, context)
- `src-tauri/src/history.rs`     kullanım geçmişi (`usage-history.jsonl`, 35 gün), grafik verisi, CSV
- `src-tauri/src/mini.rs`        her zaman üstte mini pencere (label `mini`), konum hatırlama
- `src-tauri/src/hotkey.rs`      global kısayol (varsayılan Ctrl+Alt+U)
- `src-tauri/src/status_page.rs` status.claude.com (opt-in)
- `src-tauri/src/i18n.rs`        Rust tarafı TR/EN metinler, saat biçimi
- `src-tauri/src/commands.rs`    frontend komutları
- `src`                          UI (React): `views/` Dashboard, Settings, History, About, Welcome, Mini; `main.tsx` pencere label'ına göre App veya Mini render eder
