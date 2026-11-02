# Katkı Rehberi

Katkılarınız için teşekkürler. OpenClass gerçek okullarda kullanılmak üzere
geliştiriliyor; bu yüzden kararlılık ve güvenlik, özellik sayısından önce gelir.

## Başlamadan önce

1. Geliştirme ortamını kurun: [`docs/GELISTIRME.md`](docs/GELISTIRME.md)
2. Mimari kararları okuyun: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
3. Büyük bir değişiklik planlıyorsanız önce issue açın.

## Temel kurallar

- **Sahte özellik yok.** Çalışmayan bir şey arayüzde çalışıyormuş gibi
  gösterilmez. Bağlantı yoksa sahte ekran görüntüsü gösterilmez.
- **Güvenlik sonradan eklenmez.** Ağdan gelen her komut yetki kontrolünden geçer.
- **Kullanıcıya Türkçe, loga teknik ayrıntı.** Hata mesajları kullanıcıya anlaşılır
  Türkçe iner; yığın izi ve teknik bağlam log dosyasına yazılır.
- **Hassas veri loglanmaz.** Parola, eşleştirme kodu, özel anahtar, cihaz tohumu,
  ekran içeriği hiçbir koşulda log satırına girmez.
- **Her modül mümkün olduğunca bağımsız.** Keşif, bağlantı ve ekran yayını
  ayrı katmanlardır; biri çalışmadan diğeri çalışabilmelidir.

## Kod standardı

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run typecheck
```

Bu dördü geçmeden PR açmayın.

- Rust hata yönetimi: kütüphane katmanında `thiserror`, uygulama katmanında
  `anyhow` + `.context()`. `unwrap()` yalnızca gerçekten imkânsız durumlarda ve
  gerekçesi yazılarak kullanılır.
- Yorumlar **neden** sorusunu yanıtlar, **ne** sorusunu değil. Kodun kendisi
  ne yaptığını zaten söylüyor.
- Yeni bağımlılık eklerken gerekçesini PR açıklamasında belirtin ve
  lisansının GPL-3.0 ile uyumlu olduğunu doğrulayın.

## Test

- Platformdan bağımsız mantık (protokol, filtreleme, kodlama, veritabanı)
  **birim testli olmalıdır.**
- Gerçek Windows ortamı gerektiren testler `docs/TEST-PLANI.md` içinde
  elle yürütülecek senaryo olarak işaretlenir.

## Commit mesajları

Türkçe veya İngilizce olabilir; önemli olan ne değiştiğinin ve neden
değiştiğinin anlaşılması.

## Lisans

Katkınız GPL-3.0-or-later altında lisanslanır.
