/**
 * OpenClass hata raporu formunu oluşturur.
 *
 * Kullanım: script.google.com → Yeni proje → bu dosyanın tamamını yapıştır →
 * üstten `olustur` işlevini seç → Çalıştır → izin ver. Bittiğinde
 * "Yürütme günlüğü"nde üç satır çıkar; üçünü de kopyala.
 */
function olustur() {
  var form = FormApp.create("OpenClass hata raporu");
  form.setDescription(
    "OpenClass beklenmedik biçimde kapandığında, öğretmenin onayıyla gönderilen hata raporu. " +
    "Bilgisayar adı, kullanıcı adı, IP adresi ve log satırı içermez."
  );
  // Program oturum açmadan gönderir; bu ayarlar açık olursa gönderim
  // reddedilir. Bazıları yalnızca kurum hesaplarında bulunur ve kişisel
  // hesapta "desteklenmiyor" hatası verir; kişisel hesapta zaten kapalıdırlar.
  [
    function () { form.setRequireLogin(false); },
    function () { form.setCollectEmail(false); },
    function () { form.setLimitOneResponsePerUser(false); },
    function () { form.setAllowResponseEdits(false); }
  ].forEach(function (ayar) {
    try { ayar(); } catch (e) { Logger.log("atlandı: " + e.message); }
  });

  var alanlar = [
    ["Program", false],
    ["Sürüm", false],
    ["İşletim sistemi", false],
    ["Hata mesajı", true],
    ["Konum", false],
    ["Zaman", false]
  ];
  var yanit = form.createResponse();
  alanlar.forEach(function (a) {
    var item = a[1] ? form.addParagraphTextItem() : form.addTextItem();
    item.setTitle(a[0]).setRequired(false);
    // Alan kimliklerini öğrenmek için her alana kendi adı yazılır.
    yanit.withItemResponse(item.createResponse(a[0]));
  });

  // Yanıtlar bir tabloya düşsün.
  var tablo = SpreadsheetApp.create("OpenClass hata raporları");
  form.setDestination(FormApp.DestinationType.SPREADSHEET, tablo.getId());

  Logger.log("FORM: " + form.getPublishedUrl());
  Logger.log("ALANLAR: " + yanit.toPrefilledUrl());
  Logger.log("TABLO: " + tablo.getUrl());
}
