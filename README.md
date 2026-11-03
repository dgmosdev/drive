# DGMOS Drive

DGMOS'a sürükle-bırak ile hızlı dosya yüklemek için Rust ile yazılmış masaüstü uygulaması (egui/eframe).

## Kurulum

Geliştirirken:

```bash
cargo run --release
```

macOS uygulama paketi (`DGMOS Drive.app`, ikon ve Info.plist dahil):

```bash
scripts/bundle-macos.sh
```

Çıktı `target/release/bundle/DGMOS Drive.app`. Paket ad-hoc imzalıdır; başka Mac'lere
dağıtmak için Developer ID ile imzalanıp notarize edilmesi gerekir.

Uygulama ikonu `scripts/make_icon.py` ile frontend'deki dünya logosundan üretilir:

```bash
python3 -I scripts/make_icon.py ../frontend/public assets
```

## API anahtarı

1. DGMOS → **Servisler → API Anahtarları** sayfasından **Secret** tipinde bir anahtar oluşturun.
2. İzinlere dosya yükleme rotalarını ekleyin:
   - `/api/modules/generate-presigned-upload-url-multi`
   - `/api/modules/confirm-file-upload-multi`
3. IP kısıtlaması koyduysanız uygulamayı kullanacağınız makinenin IP'sini ekleyin.
4. Uygulamayı ilk açtığınızda bağlanma ekranı gelir: erişim anahtarını ve gizli anahtarı girip
   **Bağlan**'a basın. Anahtar boş bir istekle doğrulanır, doğrulanmadan kaydedilmez.
   Sunucu adresi bu ekrandaki **Gelişmiş** bölümündedir.

> Not (Kasım 2026): rota izinleri tam path ile eşleşir; `/api` öneki zorunludur
> (`/modules/...` eşleşmez). `/api/modules/*` wildcard'ı da kabul edilir. İzin listesinde
> rota yoksa sunucu `403 Rota izni yok`, imza yanlışsa `401 Geçersiz imza` döner.
> `dg_ak_live_` öneki ortamı değil anahtar tipini belirtir. API'nin ana adresi
> `https://sandbox.dgmos.com/api`'dir (uygulamanın varsayılanı); web arayüzü bağlantıları
> `https://go.dgmos.com`'a gider.

Access Key ve API adresi `~/Library/Application Support/dgmos-drive/settings.json`
(Windows: `%APPDATA%`, Linux: `~/.config`) dosyasında durur. Secret Key yalnızca işletim
sisteminin anahtar zincirinde saklanır.

## Arayüz

- Son kullanıcı için tasarlandı: teknik ayrıntı (sunucu adresi, dosya ID'leri, HTTP kodları)
  gösterilmez; hatalar sade Türkçe mesajlara çevrilir, teknik ayrıntı yalnızca stderr'e yazılır.
- Görünüm DGMOS web arayüzüyle (Metronic 8) aynı: gri sayfa zemini, beyaz kartlar,
  `form-control-solid` girişler, `btn-primary`, `badge-light-*`, kesikli dropzone. Açık/koyu tema
  sistemi izler. Font olarak sistemde Inter varsa o, yoksa SF Pro / Segoe UI kullanılır.
- Logolar `assets/` altında gömülüdür; frontend'deki `dgmos_logo_light.png`,
  `default-small-transparent.png` ve `icon-512x512.png` dosyalarından üretildi (beyaz alfa
  maskesi, temaya göre renklendirilir).
- İnternet ve sunucu kaynaklı hatalarda dosya satırında **Tekrar dene** çıkar; anahtar geçersiz
  hale gelirse üstte **Yeniden bağlan** bandı görünür.

## Nasıl çalışır

Frontend'deki Dropzone ile aynı üç adımlı akış:

1. `generate-presigned-upload-url-multi`: her dosya için `file_id` + presigned S3 URL'i alınır
2. Dosyalar S3'e doğrudan `PUT` edilir (`x-amz-tagging: status=temp`, en fazla N eşzamanlı)
3. `confirm-file-upload-multi`: başarıyla yüklenen dosyalar onaylanır; moderasyonun
   reddettikleri (`rejected_files`) dosya bazında hata olarak gösterilir

Her API isteği `X-Access-Key`, `X-Timestamp` ve `X-Signature` başlıklarıyla gönderilir.
İmza, Secret Key ile `METHOD\nPATH\nTIMESTAMP\nBODY` metninin HMAC-SHA256 değeridir.

Dropzone'dan farkları:
- Aynı adlı iki dosya (ör. farklı klasörlerden iki `rapor.pdf`) birbirinin üzerine yazılmaz.
- Bir dosyanın S3 yüklemesi başarısız olursa diğerleri yine onaylanır.
- Klasör bırakılabilir; içi özyinelemeli taranır, gizli dosyalar atlanır.
