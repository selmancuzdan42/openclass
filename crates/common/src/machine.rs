//! Makine kimliği: bilgisayar adı.

/// Bilgisayar adını geçersiz kılan geliştirme değişkeni.
///
/// Yalnızca test içindir: tek bir makinede birden fazla sahte öğrenci
/// çalıştırabilmek için `OPENCLASS_HOSTNAME=LAB101` verilebilir.
/// Üretimde ayarlanmaz; ayarlanırsa uyarı loglanır.
pub const HOSTNAME_OVERRIDE_VAR: &str = "OPENCLASS_HOSTNAME";

/// İşletim sisteminin bildirdiği bilgisayar adını okur.
///
/// Windows'ta bu `GetComputerNameExW` üzerinden gelen NetBIOS adıdır
/// (ör. `LAB101`). Okunamazsa `UNKNOWN-HOST` döner; uygulama bu durumda
/// çökmez ama arayüzde uyarı gösterir.
pub fn hostname() -> String {
    if let Ok(forced) = std::env::var(HOSTNAME_OVERRIDE_VAR) {
        let forced = forced.trim().to_string();
        if !forced.is_empty() {
            tracing::warn!(
                %forced,
                "bilgisayar adı {HOSTNAME_OVERRIDE_VAR} ile geçersiz kılındı — \
                 bu yalnızca test içindir"
            );
            return forced;
        }
    }
    match hostname::get() {
        Ok(os) => {
            let name = os.to_string_lossy().trim().to_string();
            if name.is_empty() {
                tracing::warn!("işletim sistemi boş bilgisayar adı bildirdi");
                fallback()
            } else {
                name
            }
        }
        Err(err) => {
            tracing::warn!(%err, "bilgisayar adı okunamadı");
            fallback()
        }
    }
}

fn fallback() -> String {
    "UNKNOWN-HOST".to_string()
}

/// Bilgisayar adının karşılaştırma için normalleştirilmiş hâli.
/// Arama ve filtreleme her zaman bu biçim üzerinden yapılır.
pub fn normalized_hostname() -> String {
    hostname().to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilgisayar_adi_bos_donmez() {
        assert!(!hostname().is_empty());
    }

    #[test]
    fn normallestirilmis_ad_buyuk_harftir() {
        let n = normalized_hostname();
        assert_eq!(n, n.to_ascii_uppercase());
    }
}
