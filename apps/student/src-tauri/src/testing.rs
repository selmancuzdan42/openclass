//! Geliştirme ve test kolaylıkları.
//!
//! Bu modüldeki her şey **yalnızca hata ayıklama derlemesinde** çalışır;
//! `cfg!(debug_assertions)` kontrolü sürüm derlemesinde tüm dalları kapatır.
//!
//! Sabit bir eşleştirme kodu verildiğinde ajan, ağda gördüğü ilk öğretmenle
//! kendiliğinden eşleşir. Böylece yerel testte her sahte öğrenci için kodu
//! elle girmek gerekmez.

use crate::state::{AppState, PairIntent};

/// Sabit eşleştirme kodunu belirleyen değişken.
/// Öğretmen tarafında da aynı değişken kullanılır.
pub const PAIRING_CODE_VAR: &str = "OPENCLASS_TEST_PAIRING_CODE";

/// Yapılandırılmış sabit kod. Sürüm derlemesinde her zaman `None`.
pub fn autopair_code() -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let code = std::env::var(PAIRING_CODE_VAR).ok()?;
    let code = code.trim().to_string();
    if code.is_empty() {
        None
    } else {
        Some(code)
    }
}

/// Sabit kod ayarlıysa, keşfedilen ilk öğretmenle eşleştirme isteği kurar.
///
/// İstek kurulduysa `true` döner; bağlantı döngüsü bir sonraki turda
/// eşleştirmeyi yürütür. Henüz ağda öğretmen görünmediyse `false` döner.
pub fn try_autopair(state: &AppState) -> bool {
    let Some(code) = autopair_code() else {
        return false;
    };

    // Keşif listesinden en erken görüleni seç: aynı ağda birden çok öğretmen
    // varsa testin hangisine bağlandığı belirsiz kalmasın.
    let chosen = {
        let teachers = state.teachers.read();
        teachers.values().min_by_key(|t| t.last_seen).map(|t| {
            (
                t.host.clone(),
                t.control_port,
                t.fingerprint.clone(),
                t.teacher_name.clone(),
            )
        })
    };

    let Some((host, port, fingerprint, teacher_name)) = chosen else {
        return false;
    };

    tracing::warn!(
        %teacher_name, %host, port,
        "TEST MODU: sabit kodla kendiliğinden eşleşiliyor ({PAIRING_CODE_VAR}). \
         Bu yalnızca hata ayıklama derlemesinde çalışır."
    );

    *state.pair_intent.write() = Some(PairIntent {
        host,
        port,
        code,
        expected_fingerprint: Some(fingerprint),
    });
    true
}
