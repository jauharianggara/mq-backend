//! Sorting whitelist utk endpoint list admin.
//! Identifier TIDAK PERNAH diambil langsung dari input user — hanya dipetakan
//! lewat whitelist (anti SQL-injection), lalu di-concat ke query runtime sqlx.
//!
//! Konvensi pagination:
//! - tanpa `sort`  -> keyset cursor id (kompatibel perilaku lama: `?cursor=<id>`).
//! - dengan `sort` -> mode page/offset (`?page=1..`) — sort non-id tidak bisa
//!   pakai keyset id; volume data admin kecil sehingga offset aman.

pub struct Sort {
    /// Ekspresi SQL kolom sort — PASTI berasal dari whitelist.
    pub col: String,
    pub asc: bool,
    /// true bila user minta kolom sort eksplisit (handler harus pakai mode page/offset).
    pub custom: bool,
}

/// `allowed` = [(param_dari_FE, ekspresi_SQL), ...]; `default_col` = ekspresi id fallback.
/// `order`: "asc" (case-insensitive) urut naik, selain itu turun (default = terbaru dulu).
pub fn parse(
    sort: Option<&str>,
    order: Option<&str>,
    allowed: &[(&str, &str)],
    default_col: &str,
) -> Sort {
    match sort.and_then(|s| allowed.iter().find(|(k, _)| k.eq_ignore_ascii_case(s))) {
        Some((_, e)) => Sort {
            col: (*e).to_string(),
            asc: order.is_some_and(|o| o.eq_ignore_ascii_case("asc")),
            custom: true,
        },
        None => Sort {
            col: default_col.to_string(),
            asc: false,
            custom: false,
        },
    }
}

impl Sort {
    /// Klausa ORDER BY lengkap dengan tie-breaker id agar hasil stabil.
    pub fn order_by(&self, id_col: &str) -> String {
        let d = if self.asc { "ASC" } else { "DESC" };
        format!("ORDER BY {} {}, {} {}", self.col, d, id_col, d)
    }
}
