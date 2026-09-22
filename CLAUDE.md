# UsageTray

Windows tray app, Claude Code kullanım limitlerini gösterir. Tam spec: USAGETRAY_SPEC.md

## Kurallar
- Tauri v2 + React + TS + Rust. v1 örneği kopyalama.
- `.credentials.json` READ-ONLY. Asla yazma, asla refresh etme.
- usage API poll aralığı minimum 180 sn. Test ederken de.
- User-Agent header'ı zorunlu: claude-code/2.0.31 (tek kaynak: `src-tauri/src/config.rs`)
- Token loglama. Maskele (`credentials::mask`).
- Değişiklikten sonra `npm run tauri dev` ile çalıştığını doğrula.

## Komutlar
npm run tauri dev      # geliştirme
npm run tauri build    # installer üret
cargo test             # rust testleri
cargo run -- --probe   # credential + API doğrulama

## Yerleşim
- `src-tauri/src/config.rs`     sabitler (endpoint, header, limitler, dosya adları)
- `src-tauri/src/credentials.rs` credential okuma (salt okunur)
- `src-tauri/src/usage_api.rs`  API istemcisi, cache, backoff
- `src-tauri/src/transcripts.rs` JSONL artımlı tarama (context, bugünkü token)
- `src-tauri/src/tray.rs`       dinamik ikon, tooltip, menü, popup konumu
- `src-tauri/src/state.rs`      AppState, poll döngüsü, `usage-updated` event
- `src-tauri/src/notify.rs`     toast + dedupe
- `src`                         popup UI (React)
