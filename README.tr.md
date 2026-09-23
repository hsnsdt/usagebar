# UsageTray

**Türkçe** · [English](README.md)

Windows sistem tepsisinde yaşayan, Claude Code kullanım limitlerini gösteren küçük bir masaüstü uygulaması.
macOS'taki Usagebar'ın Windows muadili.

<p align="center">
  <img src="docs/popup.png" alt="UsageTray popup" width="340">
</p>

- **Tepsi ikonu:** 5 saatlik pencerenin doluluğu, renk kodlu halka (yeşil / sarı / turuncu / kırmızı). Tooltip'te iki pencere birden.
- **Popup:** 5 saatlik pencere için halka gösterge, canlı geri sayım ve tempo yorumu; haftalık limit ve altında modele özel haftalık limitler (ör. Fable); hesapta açıksa ekstra kullanım harcaması; kalan context (proje ve model adıyla); son 7 günün günlük token grafiği ve düne göre değişim okları.
- **Tempo işareti:** bar ve halka üstündeki küçük çizgi, pencerenin ne kadarının geçtiğini gösterir. Doluluk çizginin solundaysa rahatsın.
- **Bildirimler:** 5 saatlik pencere %50 / %75 / %90, haftalık %80 / %95, context 20K'nın altına inince. Her eşik pencere başına bir kez. İstersen yoğun geçen 5 saatlik pencere sıfırlanınca da bildirim.
- Windows ile başlar, arka planda çalışır. Boşta ~35 MB RAM, ~0 CPU. Kurulum 2.5 MB.
- Türkçe ve İngilizce. Varsayılan sistem dili; ayarlardan değiştirilebilir. Tepsi menüsü, tooltip ve bildirimler de aynı dili kullanır.

## Kurulum

1. Releases sayfasından `UsageTray_x.y.z_x64-setup.exe` dosyasını indir ve çalıştır. Kullanıcı bazlı kurulum, yönetici gerekmez. Installer'lar GitHub Actions'ta, tag'lenmiş kaynaktan derlenir; her release'te SHA256 hash'leri ve installer'sız, autostart'sız portable `.exe` de var.
2. Terminalde en az bir kez `claude` çalıştırıp giriş yapmış olman gerekir. Uygulama Claude Code'un kendi oturumunu okur, ayrı bir giriş istemez.
3. WebView2 runtime yoksa kurulum sırasında otomatik indirilir. Windows 11'de zaten vardır.

> **Windows 11 notu:** Yeni tepsi ikonları varsayılan olarak taşma menüsünde (`^`) gizlenir. Sürekli görünsün istersen ikonu oradan görev çubuğuna sürükle, ya da Ayarlar → Kişiselleştirme → Görev çubuğu → Diğer sistem tepsisi simgeleri altından aç.

Sol tık popup'ı açar, odak kaybedince ya da Esc ile kapanır. Sağ tık: Yenile / Ayarlar / Başlangıçta çalıştır / Çıkış.

## Güvenlik ve gizlilik

Bir uygulamanın OAuth token'ını okumasına şüpheyle yaklaşmak doğru. Bu yüzden:

- Token yalnızca `https://api.anthropic.com/api/oauth/usage` adresine gider. Başka hiçbir ağ isteği yok; CSP'de `connect-src` sadece bu alan adı.
- `~/.claude/.credentials.json` **salt okunur** açılır. Uygulama token yenilemez, dosyaya yazmaz; yenilemeyi Claude Code kendisi yapar.
- Token hiçbir log satırına yazılmaz; log'da `sk-ant-oat01-****` olarak maskelenir.
- Telemetri, analytics, crash reporting yok.
- Tüm veriler yerelde: `%APPDATA%\UsageTray\` (ayarlar, cache, tarama durumu, bildirim durumu, loglar).

Kod küçük ve okunabilir; şüphen varsa `src-tauri/src/usage_api.rs` ve `credentials.rs` dosyalarına bak.

Kendin doğrula:

- Release'ler geliştirici makinesinde değil, herkese açık [release workflow](.github/workflows/release.yml) ile GitHub'ın makinelerinde derlenir. Release notlarındaki SHA256'yı `Get-FileHash` ile karşılaştır.
- `cargo run -- --probe` hangi dosyanın okunduğunu ve ham API cevabını basar.
- Fiddler veya Wireshark ile trafiği izle: tek adres `api.anthropic.com`.
- Installer kod imzalı değil, o yüzden SmartScreen ilk çalıştırmada uyarı verir. Bu eksik sertifikayla ilgili, içerikle değil.

## Veri kaynakları

| Veri | Kaynak |
|---|---|
| 5 saatlik / haftalık / modele özel limit, ekstra kullanım | Anthropic OAuth usage endpoint'i. Resmi değil, topluluk keşfi; şema değişirse uygulama cache'e düşer, çökmez. |
| Kalan context, günlük token / mesaj, 7 günlük geçmiş | `~/.claude/projects/**/*.jsonl` transcript'leri. Artımlı okunur, dosya başına byte offset tutulur; `message.id + requestId` ile dedupe. |
| Plan rozeti | `.credentials.json` içindeki `subscriptionType` |

API en fazla 5 dakikada bir sorgulanır. Aralık ayarlardan artırılabilir, 180 saniyenin altına inemez. PC uykudan uyanınca bir sonraki sorgu öne çekilir (yine de bir öncekine 180 saniyeden yakın olmaz). 429 alınırsa üstel geri çekilme uygulanır (5 → 10 → 20 → 30 dk) ve son bilinen veri "bayat" işaretiyle gösterilir.

Context hesabı son assistant mesajının `input + cache_read + cache_creation + output` toplamıdır. Pencere, oturumun model adından belirlenir (Claude 5 ailesi 1M, öncekiler 200K) ve autocompact payı düşülür. Bir oturum varsayılan pencereyi aşarsa uygulama bir üst kademeye geçer, %100'de takılı kalmaz. Ayarlardan otomatik modu kapatıp kendi değerini yazabilirsin.

## Durumlar

| Popup'ta gördüğün | Anlamı |
|---|---|
| Claude Code bulunamadı | Credential dosyası yok. Terminalde `claude` çalıştırıp giriş yap. |
| Token süresi dolmuş | Access token ~1 saatte dolar ve sadece Claude Code çalışırken yenilenir. Bir kez `claude` çalıştırman yeter; hata değil. |
| Hız sınırı | 429 alındı, X dk sonra tekrar denenecek. |
| Çevrimdışı | Bağlantı yok, cache gösteriliyor. |

Yerel veriler (context, günlük istatistikler) API'den bağımsızdır; bu durumlarda da gösterilmeye devam eder.

## Ayarlar

Popup içinde dişli ikonu. Yenileme aralığı, otomatik veya elle context penceresi, tema (koyu / açık / sistem), dil (sistem / Türkçe / English), saat biçimi (sistem / 24 saat / 12 saat), kullanılan veya kalan yüzde, tepsi ikonunda yüzde yazısı, Windows ile başlatma, bildirim eşikleri, reset bildirimi. Dosya: `%APPDATA%\UsageTray\settings.json`; bozuksa varsayılanlara dönülür.

## Geliştirme

Gereksinimler: Rust (stable, MSVC), Node 20+, Visual Studio Build Tools (C++ iş yükü), WebView2.

```
npm install
npm run tauri dev        # geliştirme (hot reload)
npm run tauri build      # NSIS installer: target/release/bundle/nsis/
cargo test               # Rust birim testleri
cargo run -- --probe     # credential + API doğrulama, ham JSON'u basar
```

Hata ayıklama: `usagetray.exe --show` popup'ı açılışta gösterir, `--settings` ayarlar sekmesiyle açar, `--pin` odak kaybında gizlemeyi kapatır (ekran görüntüsü için). `USAGETRAY_LOG=debug` ayrıntılı log verir. `APPDATA` değişkenini başka bir klasöre yönlendirerek temiz bir ilk açılış denenebilir.

Yerleşim:

```
src/                      popup UI (React + TS, plain CSS); src/i18n.ts metinler
src-tauri/src/i18n.rs     Rust tarafı metinler (menü, tooltip, bildirim)
src-tauri/src/config.rs   sabitler: endpoint, header'lar, limitler
src-tauri/src/context.rs  modele göre context penceresi
src-tauri/src/credentials.rs   credential okuma (salt okunur)
src-tauri/src/usage_api.rs     API istemcisi, cache, backoff
src-tauri/src/transcripts.rs   JSONL artımlı tarama, 7 günlük geçmiş
src-tauri/src/tray.rs          dinamik ikon, menü, popup konumu
src-tauri/src/state.rs         AppState, poll döngüsü, usage-updated event
src-tauri/src/toasts.rs        bildirimler ve dedupe
```

Stack: Tauri v2, Rust, React 19, Vite. İkon tiny-skia ile runtime'da çizilir. Tam spec: `USAGETRAY_SPEC.md`, çalışma notları: `CLAUDE.md`.

## Lisans

Geliştiren: Sedat Okutan ([hsnsdt](https://github.com/hsnsdt), sedat@okutan.org, [www.okutan.org](https://www.okutan.org)).

MIT. Gömülü Inter fontu SIL Open Font License (`src-tauri/assets/Inter-LICENSE.txt`).
