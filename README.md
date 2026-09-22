# UsageTray

Windows sistem tepsisinde yaşayan, Claude Code kullanım limitlerini gösteren küçük bir masaüstü uygulaması.
macOS'taki Usagebar'ın Windows muadili.

- Tepsi ikonu: 5 saatlik pencerenin doluluğu, renk kodlu halka (yeşil / sarı / turuncu / kırmızı)
- Tıklayınca açılan panel: 5 saatlik limit, haftalık limit, canlı reset sayaçları, kalan context, bugünkü mesaj ve token sayısı
- Eşik bildirimleri: 5 saatlik pencere %50 / %75 / %90, haftalık %80 / %95, context 20K altına inince
- Windows ile başlar, arka planda çalışır, boşta ~0 CPU

## Kurulum

1. Releases sayfasından `UsageTray_x.y.z_x64-setup.exe` dosyasını indir ve çalıştır (kullanıcı bazlı kurulum, yönetici gerekmez).
2. Terminalde en az bir kez `claude` çalıştırıp giriş yapmış olman gerekir. Uygulama Claude Code'un kendi oturumunu okur, ayrı bir giriş istemez.
3. WebView2 runtime yoksa kurulum sırasında otomatik indirilir (Windows 11'de zaten vardır).

> **Windows 11 notu:** Yeni tepsi ikonları varsayılan olarak taşma menüsünde (`^`) gizlenir. Sürekli görünsün istersen ikonu oradan görev çubuğuna sürükle, ya da Ayarlar → Kişiselleştirme → Görev çubuğu → Diğer sistem tepsisi simgeleri altından aç.

## Güvenlik ve gizlilik

Bir uygulamanın OAuth token'ını okumasına şüpheyle yaklaşmak doğru. Bu yüzden:

- Token yalnızca `https://api.anthropic.com/api/oauth/usage` adresine gider. Başka hiçbir ağ isteği yok, CSP'de de `connect-src` sadece bu alan adı.
- `~/.claude/.credentials.json` **salt okunur** açılır. Uygulama token yenilemez, dosyaya yazmaz; yenilemeyi Claude Code kendisi yapar.
- Token hiçbir log satırına yazılmaz; log'da `sk-ant-oat01-****` olarak maskelenir.
- Telemetri, analytics, crash reporting yok.
- Tüm veriler yerelde: `%APPDATA%\UsageTray\` (ayarlar, cache, tarama durumu, loglar).

## Veri kaynakları

| Veri | Kaynak |
|---|---|
| 5 saatlik / haftalık limit | Anthropic OAuth usage endpoint'i (resmi değil, topluluk keşfi; şema değişirse uygulama cache'e düşer, çökmez) |
| Kalan context, bugünkü token/mesaj | `~/.claude/projects/**/*.jsonl` transcript'leri, artımlı okunur |
| Plan rozeti | `.credentials.json` içindeki `subscriptionType` |

API en fazla 5 dakikada bir sorgulanır (ayarlardan artırılabilir, 180 sn'nin altına inemez). 429 alınırsa üstel geri çekilme uygulanır ve son bilinen veri "bayat" işaretiyle gösterilir.

## Durumlar

| Panelde gördüğün | Anlamı |
|---|---|
| Claude Code bulunamadı | Credential dosyası yok. Terminalde `claude` çalıştırıp giriş yap. |
| Token süresi dolmuş | Access token ~1 saatte dolar ve sadece Claude Code çalışırken yenilenir. Bir kez `claude` çalıştırman yeter, hata değil. |
| Anthropic hız sınırı | 429 alındı, X dk sonra tekrar denenecek. |
| Bağlantı yok | Çevrimdışısın, cache gösteriliyor. |

Yerel veriler (context, bugünkü token) API'den bağımsızdır, bu durumlarda da gösterilmeye devam eder.

## Geliştirme

Gereksinimler: Rust (stable, MSVC), Node 20+, Visual Studio Build Tools (C++), WebView2.

```
npm install
npm run tauri dev        # geliştirme (hot reload)
npm run tauri build      # NSIS installer: src-tauri/target/release/bundle/nsis/
cargo test               # Rust birim testleri
cargo run -- --probe     # credential + API doğrulama, ham JSON'u basar
```

Hata ayıklama bayrakları: `usagetray.exe --show` popup'ı açılışta gösterir, `--settings` ayarlar sekmesiyle açar.
`USAGETRAY_LOG=debug` ile ayrıntılı log alınır.

Yerleşim ve kurallar için `CLAUDE.md`, tam spec için `USAGETRAY_SPEC.md`.

## Lisans

Uygulama kodu MIT. Gömülü Inter fontu SIL Open Font License (bkz. `src-tauri/assets/Inter-LICENSE.txt`).
