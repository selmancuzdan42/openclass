# Arayüz Tasarımı

## Neden bu yerleşim?

OpenClass'ın öğretmen konsolu bir web panosu değil, öğretmenin ders boyunca
önünde duracağı bir masaüstü aracıdır. Bu yüzden yerleşim, sınıf yönetimi
yazılımlarının oturmuş bilgi mimarisini izler (NetSupport School'un öğretmen
konsolu referans alındı), görsel dil ise moderndir — eski nesil Office şeridi
taklit edilmez.

```
┌──────────────────────────────────────────────────────────────────────┐
│ Başlık çubuğu   OpenClass · sınıf adı · ders süresi · hızlı eylemler │
├──────────────────────────────────────────────────────────────────────┤
│ KOMUT ŞERİDİ    [Sınıf] [İzleme] [Seçim] [Yönetim]                   │
│                 ikonlu komutlar, adlandırılmış gruplar hâlinde       │
├──────────────────────────────────────────────────────────────────────┤
│ GRUP ÇUBUĞU     ● Tümü 13/16   ● Lab 1 10/12   ● Lab 2 3/4   + Grup  │
├──────────────────────────────────────────────────────────────────────┤
│ ARAMA           [ Laboratuvar veya bilgisayar adı ] [Bilgisayarları  │
│                                                       Ara]           │
├────────┬─────────────────────────────────────────────────────────────┤
│ İzleme │                                                             │
│ Yardım │   CİHAZ LİSTESİ                                             │
│ Cihazl.│   küçük resim ızgarası  ↔  ayrıntılı tablo                  │
│ Kayıtl.│                                                             │
├────────┴─────────────────────────────────────────────────────────────┤
│ DURUM ÇUBUĞU  ● Lab 1 │ Bağlı 13 │ Seçili 0 │ Grupta 16 │ Seç: ⋯ │ ⊞ ≣│
└──────────────────────────────────────────────────────────────────────┘
```

### Her bölgenin işi

| Bölge | İçerik | Neden |
|---|---|---|
| **Başlık çubuğu** | Sınıf adı, ders süresi, "Dersi Başlat/Bitir", yardım talebi sayacı | En sık kullanılan iki eylem her sekmeden erişilebilir olmalı |
| **Komut şeridi** | Sekmelere ve adlandırılmış gruplara ayrılmış komutlar | Öğretmen aradığı komutu ezberden bulur; menü avı yok |
| **Grup çubuğu** | Laboratuvar sekmeleri, her biri `bağlı/toplam` sayısıyla | Hangi laboratuvarın ne durumda olduğu tek bakışta |
| **Arama satırı** | Ön ek araması (`LAB1`) | Şartnamenin açık gereksinimi; grup çubuğuyla birlikte çalışır |
| **Sol ray** | Görünüm modları: İzleme, Yardım, Cihazlar, Kayıtlar | Komut (şerit) ile görünüm (ray) ayrımı net kalsın |
| **Liste** | Küçük resim ızgarası veya ayrıntılı tablo | 20 ekranı izlemek ile 20 kaydı yönetmek farklı işler |
| **Durum çubuğu** | Sayaçlar, seçim kısayolları, görünüm anahtarı, boyut kaydırıcısı | Ders sırasında en çok dokunulan ayarlar elin altında |

## Küçük resim kartı

```
┌─────────────────────────┐
│ ⚑Yardım                 │   ← durum rozetleri
│                         │
│     ekran görüntüsü     │   ← gerçek kare veya nedeni açıklayan yer tutucu
│                         │
│  14 sn         480×270  │   ← bayatlık ve çözünürlük
├─────────────────────────┤
│ ● LAB101     1. sıra sol│   ← ad şeridi; seçiliyken vurgu rengine döner
└─────────────────────────┘
```

* Çift tıklama büyütülmüş görünümü açar ve o cihaz yüksek kaliteye geçer.
* Sağ tık cihaza özgü menüyü getirir.
* Tek tık seçer; `Ctrl`/`Shift` ile çoklu seçim yapılır.
* **Gerçek kare gelmediği sürece sahte görüntü çizilmez.** Yer tutucu nedeni
  yazar: çevrimdışı, yayın kapalı, yetki bekliyor veya yakalama hatası.

## Gruplar

Grup, bilgisayar adı ön ekinden ibarettir: `LAB1` grubu `LAB101`, `LAB102` …
cihazlarını kapsar. Hiçbir adlandırma düzeni koda gömülü değildir; `BIL101`,
`PC-101`, `ATOLYE01`, `SINIF-A-01` düzenleri de çalışır.

Grup eklerken konsol, ağdaki bilgisayar adlarından ön ek **önerir**: adların
sonundaki rakamlar kademeli olarak atılır ve en az iki cihazı kapsayan adaylar
cihaz sayısıyla birlikte listelenir.

## Renk ve yoğunluk

* Tipografi 10–13 px, satır yükseklikleri 24–32 px, köşe yarıçapı 4–6 px.
  Yoğunluk bilinçlidir: 20 ekran bir pencereye sığmalı.
* Renk belirteçleri `styles.css` içinde tek yerde tanımlıdır; açık ve koyu
  tema aynı değişkenleri yeniden tanımlar.
* Durum renkleri tek anlamlıdır: yeşil = bağlı, turuncu = ağda ama bağlı değil
  / yardım talebi, kırmızı = hata veya yetki kaldırıldı, gri = çevrimdışı.
* Hareket azaltma tercihine (`prefers-reduced-motion`) uyulur.

## Öğrenci ajanı

Öğrenci tarafında **pencere yoktur.** Ajan sistem tepsisinde çalışır; bu,
sınıf yönetimi yazılımlarının yerleşik modelidir (NetSupport'ta da öğrenci
ajanı tepside durur, ayarlar ayrı bir yapılandırma aracındadır).

### Öğrencinin gördüğü tek yüzey: tepsi simgesi

| Simge | Anlamı |
|---|---|
| Ekran çizimi, soluk + kırmızı çapraz | Bağlı değil |
| Ekran çizimi, net | Öğretmene bağlı, paylaşım kapalı |
| Ekran çizimi + **yeşil nokta** | **Ekranınız paylaşılıyor** |

Simgenin üzerine gelince ipucu metni bilgisayar adını, bağlantı durumunu ve
paylaşım durumunu yazar. Sağ tık menüsü üç şey içerir:

```
Bağlı — Cihan Güngör  ·  EKRANINIZ PAYLAŞILIYOR     (bilgi, tıklanamaz)
────────────────────────────────────────────────
Yardım İste                                        (bağlıyken etkin)
────────────────────────────────────────────────
Yapılandırma…
```

### Yapılandırma penceresi — öğretmen/yönetici içindir

Normal kullanımda **gizlidir**; yalnızca tepsi menüsünden veya simgeye çift
tıklayarak açılır. İçinde yalnızca kurulum ayarları vardır: bu bilgisayarın
kimliği, bağlantı durumu, öğretmen eşleştirme (veya eşleşilmiş öğretmenin
bilgisi), sabit öğretmen adresi ve ajanı kapatma.

Pencere kapatıldığında ajan durmaz, yalnızca gizlenir.

### Mesaj penceresi

Öğretmen mesaj gönderdiğinde küçük bir pencere açılır, öğrenci okuyup
**Tamam**'a basar. Mesajlar kalıcı bir panelde birikmez — öğrencinin
ekranında sürekli duran bir OpenClass penceresi yoktur.

### Neden "Quiet Mode" yok?

NetSupport'ta öğrenci ajanının varlığını tamamen gizleyen bir seçenek var.
OpenClass'ta **yoktur ve eklenmeyecektir**: ekranı paylaşılan kişinin bunu
görebilmesi ürünün temel ilkesidir. Tepsi simgesi her zaman görünür ve
paylaşım sırasında belirgin biçimde değişir.

### Neden tepsi menüsünde "Çıkış" yok?

Ders ortasında öğrencinin tek tıkla izlemeyi kapatabilmesi öğretmen için
işlevsel bir gerileme olurdu. Ajanı durdurma düğmesi yapılandırma
penceresindedir. Bu bir kurcalama koruması **değildir** — görev
yöneticisinden süreç yine sonlandırılabilir; bunu engellemek işletim sistemi
ilkesi gerektirir ve bu projenin kapsamı dışındadır.

## Arayüzü Rust derlemeden geliştirme

Her iki uygulama da Tauri dışında, düz bir tarayıcıda sahte veriyle açılabilir:

```bash
npm run dev --workspace @openclass/teacher   # http://127.0.0.1:1420
npm run dev --workspace @openclass/student   # http://127.0.0.1:1421
npm run dev --workspace @openclass/student   # ?view=notice → mesaj penceresi
```

Tauri köprüsü bulunmadığında `src/dev-mock.ts` devreye girer ve örnek bir
sınıf (LAB101–LAB204, yardım talepleri, açık ders oturumu) üretir. Bu kod
`import.meta.env.DEV` ile korunur; üretim derlemesine girmez.

Başsız tarayıcıyla ekran görüntüsü alırken adrese `?shot=1` ekleyin; geliştirme
sunucusu `load` olayını kısa süre geciktirir, böylece görüntü veriler
yerleştikten sonra alınır.
