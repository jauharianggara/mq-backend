//! Service modul visits v2 (Panggil Ustadz — ketersediaan mingguan, deposit, durasi jam).
//! Semua query: kolom tanggal = DATE_FORMAT -> String (gotcha sqlx TIMESTAMP);
//! tuple query_as <= 16 kolom (gotcha sqlx tuple).
use chrono::Datelike;
use chrono::Timelike;
use sqlx::MySqlPool;

use crate::modules::visits::dto::*;
use crate::modules::visits::payments;
use crate::shared::error::AppError;
use crate::state::AppState;

pub fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}

pub const MIN_TARIF_PER_JAM: i64 = 10_000;

// ===================== helpers =====================

pub async fn setting_str(pool: &MySqlPool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT CAST(value AS CHAR) FROM settings WHERE `key` = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(|s| s.trim().trim_matches('"').to_string())
}

pub async fn setting_i64(pool: &MySqlPool, key: &str, default: i64) -> i64 {
    setting_str(pool, key)
        .await
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(default)
}

pub async fn visit_enabled(pool: &MySqlPool) -> Result<bool, AppError> {
    Ok(setting_str(pool, "visit_enabled").await.as_deref() == Some("true"))
}

pub fn haversine_km(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let (lat1, lng1, lat2, lng2) = (lat1.to_radians(), lng1.to_radians(), lat2.to_radians(), lng2.to_radians());
    let dlat = lat2 - lat1;
    let dlng = lng2 - lng1;
    let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlng / 2.0).sin().powi(2);
    6371.0 * 2.0 * a.sqrt().asin()
}

pub async fn notify<'e, E>(ex: E, user_id: i64, code: &str, title: &str, body: &str, deeplink: &str) -> Result<(), AppError>
where
    E: sqlx::Executor<'e, Database = sqlx::MySql>,
{
    let data = format!("{{\"deeplink\":\"visit:{deeplink}\"}}");
    sqlx::query(
        "INSERT INTO user_notifications (user_id, template_code, title, body, data, channel) \
         VALUES (?, ?, ?, ?, CAST(? AS JSON), 'IN_APP')")
        .bind(user_id).bind(code).bind(title).bind(body).bind(data)
        .execute(ex).await.map_err(dberr)?;
    Ok(())
}

pub async fn log_history<'e, E>(ex: E, visit_id: i64, from: Option<&str>, to: &str, actor: Option<i64>, note: &str) -> Result<(), AppError>
where
    E: sqlx::Executor<'e, Database = sqlx::MySql>,
{
    sqlx::query(
        "INSERT INTO ustadz_visit_status_history (visit_id, from_status, to_status, actor_id, note) VALUES (?, ?, ?, ?, ?)")
        .bind(visit_id).bind(from).bind(to).bind(actor).bind(note)
        .execute(ex).await.map_err(dberr)?;
    Ok(())
}

pub async fn notify_admins(pool: &MySqlPool, title: &str, body: &str) {
    let ids: Vec<(i64,)> = sqlx::query_as(
        "SELECT DISTINCT ur.user_id FROM user_roles ur JOIN roles r ON r.id = ur.role_id \
         WHERE r.code IN ('ADMIN','SUPER_ADMIN') LIMIT 20")
        .fetch_all(pool).await.unwrap_or_default();
    for (uid,) in ids {
        if let Ok(mut conn) = pool.acquire().await {
            let _ = notify(&mut *conn, uid, "KHATMIL_ASSIGN_RESULT", title, body, "").await;
        }
    }
}

// ===================== visit row =====================

pub struct VisitRow {
    pub id: i64,
    pub user_id: i64,
    pub ustadz_id: i64,
    pub scheduled_at: String, // UTC "YYYY-MM-DD HH:MM:SS"
    pub duration_hours: i64,
    pub lat: f64,
    pub lng: f64,
    pub address_label: String,
    pub note: Option<String>,
    pub price_per_hour: i64,
    pub price_total: i64,
    pub anonymized: bool,
    pub status: String,
    pub cancel_reason: Option<String>,
    pub decline_reason: Option<String>,
    pub created_at: String,
    pub paid_at: Option<String>,
    pub confirmed_at: Option<String>,
    pub completed_at: Option<String>,
    pub canceled_at: Option<String>,
}

const VISIT_COLS_A: &str =
    "v.id, v.user_id, v.ustadz_id, DATE_FORMAT(v.scheduled_at, '%Y-%m-%d %H:%i:%s'), v.duration_hours, \
     CAST(v.lat AS DOUBLE), CAST(v.lng AS DOUBLE), v.address_label, v.note, v.price_per_hour, v.price_total, \
     v.anonymized, v.status, v.cancel_reason, v.decline_reason";
const VISIT_COLS_B: &str =
    "DATE_FORMAT(v.created_at, '%Y-%m-%d %H:%i:%s'), DATE_FORMAT(v.paid_at, '%Y-%m-%d %H:%i:%s'), \
     DATE_FORMAT(v.confirmed_at, '%Y-%m-%d %H:%i:%s'), DATE_FORMAT(v.completed_at, '%Y-%m-%d %H:%i:%s'), \
     DATE_FORMAT(v.canceled_at, '%Y-%m-%d %H:%i:%s')";

type VisitRowA = (i64, i64, i64, String, i64, f64, f64, String, Option<String>, i64, i64, i8, String, Option<String>, Option<String>);
type VisitRowB = (String, Option<String>, Option<String>, Option<String>, Option<String>);

fn map_visit(a: VisitRowA, b: VisitRowB) -> VisitRow {
    VisitRow {
        id: a.0, user_id: a.1, ustadz_id: a.2, scheduled_at: a.3, duration_hours: a.4,
        lat: a.5, lng: a.6, address_label: a.7, note: a.8,
        price_per_hour: a.9, price_total: a.10, anonymized: a.11 != 0, status: a.12,
        cancel_reason: a.13, decline_reason: a.14,
        created_at: b.0, paid_at: b.1, confirmed_at: b.2, completed_at: b.3, canceled_at: b.4,
    }
}

/// Ambil visit terpisah 2 query (<= 16 kolom per query).
pub async fn fetch_visit(pool: &MySqlPool, id: i64) -> Result<Option<VisitRow>, AppError> {
    let a: Option<VisitRowA> = sqlx::query_as(
        &format!("SELECT {VISIT_COLS_A} FROM ustadz_visits v WHERE v.id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(dberr)?;
    let a = match a { Some(a) => a, None => return Ok(None) };
    let b: VisitRowB = sqlx::query_as(
        &format!("SELECT {VISIT_COLS_B} FROM ustadz_visits v WHERE v.id = ?"))
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(dberr)?;
    Ok(Some(map_visit(a, b)))
}

async fn fetch_visit_tx(tx: &mut sqlx::Transaction<'_, sqlx::MySql>, id: i64) -> Result<Option<VisitRow>, AppError> {
    let a: Option<VisitRowA> = sqlx::query_as(
        &format!("SELECT {VISIT_COLS_A} FROM ustadz_visits v WHERE v.id = ? FOR UPDATE"))
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(dberr)?;
    let a = match a { Some(a) => a, None => return Ok(None) };
    let b: VisitRowB = sqlx::query_as(
        &format!("SELECT {VISIT_COLS_B} FROM ustadz_visits v WHERE v.id = ? FOR UPDATE"))
        .bind(id)
        .fetch_one(&mut **tx)
        .await
        .map_err(dberr)?;
    Ok(Some(map_visit(a, b)))
}

pub struct PaymentRow {
    pub id: i64,
    pub external_id: String,
    pub invoice_id: Option<String>,
    pub amount: i64,
    pub status: String,
    pub refunded_amount: i64,
    pub subject_id: i64,
    pub channel: Option<String>,
}

pub async fn fetch_payment(pool: &MySqlPool, visit_id: i64) -> Result<Option<PaymentRow>, AppError> {
    let row: Option<(i64, String, Option<String>, i64, String, i64, i64, Option<String>)> = sqlx::query_as(
        "SELECT id, external_id, xendit_invoice_id, amount, status, refunded_amount, subject_id, channel \
         FROM payments WHERE subject_type = 'ustadz_visit' AND subject_id = ? ORDER BY id DESC LIMIT 1")
        .bind(visit_id).fetch_optional(pool).await.map_err(dberr)?;
    Ok(row.map(|r| PaymentRow { id: r.0, external_id: r.1, invoice_id: r.2, amount: r.3, status: r.4, refunded_amount: r.5, subject_id: r.6, channel: r.7 }))
}

async fn party(pool: &MySqlPool, user_id: i64) -> PartyOut {
    let (name, phone): (String, Option<String>) = sqlx::query_as(
        "SELECT COALESCE(NULLIF(up.full_name,''),'Tanpa Nama'), u.phone \
         FROM users u LEFT JOIN user_profiles up ON up.user_id = u.id WHERE u.id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or(("Tanpa Nama".into(), None));
    PartyOut { user_id, full_name: name, phone }
}

fn contact_open(status: &str) -> bool {
    matches!(status, "CONFIRMED" | "COMPLETED" | "REVIEWED")
}

pub async fn visit_out(pool: &MySqlPool, v: &VisitRow, viewer: Option<i64>) -> Result<VisitOut, AppError> {
    let is_admin = viewer.is_none();
    let payment = fetch_payment(pool, v.id).await?;
    let mut ustadz = party(pool, v.ustadz_id).await;
    let mut requester = party(pool, v.user_id).await;
    if !contact_open(&v.status) && !is_admin {
        ustadz.phone = None;
        requester.phone = None;
    }
    let show_coords = is_admin || viewer.is_some();
    Ok(VisitOut {
        id: v.id,
        status: v.status.clone(),
        scheduled_at: v.scheduled_at.clone(),
        duration_hours: v.duration_hours,
        price_per_hour: v.price_per_hour,
        price_total: v.price_total,
        address_label: if v.anonymized { "—".into() } else { v.address_label.clone() },
        note: v.note.clone(),
        lat: if show_coords && !v.anonymized { Some(v.lat) } else { None },
        lng: if show_coords && !v.anonymized { Some(v.lng) } else { None },
        anonymized: v.anonymized,
        ustadz: Some(ustadz),
        requester: Some(requester),
        payment: payment.map(|p| PaymentOut {
            id: p.id, external_id: p.external_id, status: p.status,
            invoice_url: None, amount: p.amount, refunded_amount: p.refunded_amount,
            channel: p.channel, expires_at: None, paid_at: None,
        }),
        created_at: v.created_at.clone(),
        paid_at: v.paid_at.clone(),
        confirmed_at: v.confirmed_at.clone(),
        completed_at: v.completed_at.clone(),
        canceled_at: v.canceled_at.clone(),
        cancel_reason: v.cancel_reason.clone(),
        decline_reason: v.decline_reason.clone(),
    })
}

pub async fn visit_detail_out(pool: &MySqlPool, v: &VisitRow, viewer: Option<i64>) -> Result<VisitOut, AppError> {
    visit_out(pool, v, viewer).await
}

// ===================== ketersediaan (ustadz) =====================

pub async fn get_availability(pool: &MySqlPool, ustadz_id: i64) -> Result<serde_json::Value, AppError> {
    let slots: Vec<(i64, i8, i64, i64)> = sqlx::query_as(
        "SELECT id, weekday, start_minute, end_minute FROM ustadz_availability_slots \
         WHERE ustadz_id = ? ORDER BY FIELD(weekday,1,2,3,4,5,6,0), start_minute")
        .bind(ustadz_id).fetch_all(pool).await.map_err(dberr)?;
    let blackouts: Vec<(chrono::NaiveDate, Option<String>)> = sqlx::query_as(
        "SELECT off_date, note FROM ustadz_blackout_dates WHERE ustadz_id = ? ORDER BY off_date")
        .bind(ustadz_id).fetch_all(pool).await.map_err(dberr)?;
    Ok(serde_json::json!({
        "slots": slots.iter().map(|s| serde_json::json!({
            "id": s.0, "weekday": s.1,
            "start": format!("{:02}:{:02}", s.2 / 60, s.2 % 60),
            "end": format!("{:02}:{:02}", s.3 / 60, s.3 % 60),
        })).collect::<Vec<_>>(),
        "blackouts": blackouts.iter().map(|b| serde_json::json!({
            "off_date": b.0.format("%Y-%m-%d").to_string(),
            "note": b.1,
        })).collect::<Vec<_>>(),
    }))
}

pub async fn add_slot(pool: &MySqlPool, ustadz_id: i64, weekday: i8, start_minute: i16, end_minute: i16) -> Result<i64, AppError> {
    if !(0..=6).contains(&weekday) {
        return Err(AppError::Unprocessable("weekday 0-6 (0=Minggu)".into()));
    }
    if !(0..1439).contains(&start_minute) || !(1..=1440).contains(&end_minute) || end_minute <= start_minute {
        return Err(AppError::Unprocessable("rentang jam tidak valid (end > start)".into()));
    }
    if end_minute - start_minute < 60 {
        return Err(AppError::Unprocessable("rentang minimal 1 jam".into()));
    }
    let dup: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ustadz_availability_slots WHERE ustadz_id = ? AND weekday = ? \
         AND NOT (end_minute <= ? OR start_minute >= ?)")
        .bind(ustadz_id).bind(weekday).bind(start_minute).bind(end_minute)
        .fetch_one(pool).await.map_err(dberr)?;
    if dup > 0 {
        return Err(AppError::Conflict("rentang jam tumpang tindih dgn slot lain".into()));
    }
    let cnt: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ustadz_availability_slots WHERE ustadz_id = ? AND weekday = ?")
        .bind(ustadz_id).bind(weekday).fetch_one(pool).await.map_err(dberr)?;
    if cnt >= 3 {
        return Err(AppError::Unprocessable("maksimal 3 rentang per hari".into()));
    }
    let ins = sqlx::query(
        "INSERT INTO ustadz_availability_slots (ustadz_id, weekday, start_minute, end_minute) VALUES (?, ?, ?, ?)")
        .bind(ustadz_id).bind(weekday).bind(start_minute).bind(end_minute)
        .execute(pool).await.map_err(dberr)?;
    Ok(ins.last_insert_id() as i64)
}

pub async fn delete_slot(pool: &MySqlPool, ustadz_id: i64, slot_id: i64) -> Result<(), AppError> {
    let n = sqlx::query("DELETE FROM ustadz_availability_slots WHERE id = ? AND ustadz_id = ?")
        .bind(slot_id).bind(ustadz_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 { return Err(AppError::NotFound("slot tidak ada".into())); }
    Ok(())
}

pub async fn add_blackout(pool: &MySqlPool, ustadz_id: i64, off_date: &str, note: Option<String>) -> Result<(), AppError> {
    let d = chrono::NaiveDate::parse_from_str(off_date, "%Y-%m-%d")
        .map_err(|_| AppError::Unprocessable("tanggal YYYY-MM-DD".into()))?;
    sqlx::query("INSERT IGNORE INTO ustadz_blackout_dates (ustadz_id, off_date, note) VALUES (?, ?, ?)")
        .bind(ustadz_id).bind(d).bind(note.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}

pub async fn delete_blackout(pool: &MySqlPool, ustadz_id: i64, off_date: &str) -> Result<(), AppError> {
    let d = chrono::NaiveDate::parse_from_str(off_date, "%Y-%m-%d")
        .map_err(|_| AppError::Unprocessable("tanggal YYYY-MM-DD".into()))?;
    let n = sqlx::query("DELETE FROM ustadz_blackout_dates WHERE ustadz_id = ? AND off_date = ?")
        .bind(ustadz_id).bind(d)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 { return Err(AppError::NotFound("tanggal libur tidak ada".into())); }
    Ok(())
}

// ===================== nearby & slots (santri) =====================

pub async fn nearby(state: &AppState, lat: f64, lng: f64) -> Result<Vec<NearbyUstadz>, AppError> {
    if !visit_enabled(&state.pool).await? {
        return Err(AppError::Forbidden("modul Pesan Ustadz sedang nonaktif".into()));
    }
    let radius = setting_i64(&state.pool, "visit_radius_km", 5).await.clamp(1, 50) as f64;
    let dlat = radius / 111.0;
    let dlng = radius / (111.0 * lat.to_radians().cos().max(0.2));
    let rows: Vec<(i64, String, f64, f64, i64, Option<f64>, i64)> = sqlx::query_as(&format!(
        "SELECT u.id, COALESCE(NULLIF(upn.full_name,''),'Ustadz'), CAST(up.point_lat AS DOUBLE), CAST(up.point_lng AS DOUBLE), \
         vs.price_per_hour, \
         (SELECT CAST(AVG(vr.rating) AS DOUBLE) FROM visit_reviews vr WHERE vr.reviewee_id = u.id \
            AND vr.direction = 'SANTRI_TO_USTADZ' AND vr.revealed_at IS NOT NULL AND vr.hidden = 0), \
         (SELECT COUNT(*) FROM visit_reviews vr2 WHERE vr2.reviewee_id = u.id \
            AND vr2.direction = 'SANTRI_TO_USTADZ' AND vr2.revealed_at IS NOT NULL AND vr2.hidden = 0) \
         FROM users u \
         JOIN user_profiles upn ON upn.user_id = u.id \
         JOIN ustadz_profiles up ON up.user_id = u.id AND up.verified_at IS NOT NULL \
         JOIN ustadz_visit_settings vs ON vs.ustadz_id = u.id AND vs.is_accepting = 1 \
         WHERE u.status = 'ACTIVE' \
           AND up.point_lat IS NOT NULL AND up.point_lng IS NOT NULL 
           AND up.point_lat BETWEEN ? AND ? AND up.point_lng BETWEEN ? AND ? \
           AND (SELECT COUNT(*) FROM ustadz_availability_slots s WHERE s.ustadz_id = u.id) > 0"))
        .bind(lat - dlat).bind(lat + dlat)
        .bind(lng - dlng).bind(lng + dlng)
        .fetch_all(&state.pool)
        .await
        .map_err(dberr)?;
    let mut out: Vec<NearbyUstadz> = rows.into_iter().map(|r| {
        let dist = haversine_km(lat, lng, r.2, r.3);
        NearbyUstadz {
            ustadz_id: r.0, full_name: r.1, distance_km: dist,
            rating_avg: r.5, rating_count: r.6, price_per_hour: r.4,
        }
    }).filter(|u| u.distance_km <= radius).collect();
    out.sort_by(|a, b| a.distance_km.partial_cmp(&b.distance_km).unwrap_or(std::cmp::Ordering::Equal));
    Ok(out)
}

/// Daftar jam mulai (WIB, bulat) yang tersedia utk tanggal & durasi tertentu.
pub async fn slots_for_date(
    pool: &MySqlPool,
    ustadz_id: i64,
    date: &str,
    hours: i64,
) -> Result<(Vec<String>, Vec<(String, i64)>), AppError> {
    let d = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| AppError::Unprocessable("date YYYY-MM-DD".into()))?;
    let weekday = d.weekday().num_days_from_sunday() as i8; // 0=Minggu
    let bo: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ustadz_blackout_dates WHERE ustadz_id = ? AND off_date = ?")
        .bind(ustadz_id).bind(d).fetch_one(pool).await.map_err(dberr)?;
    if bo > 0 { return Ok((vec![], vec![])); }
    let slots: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT start_minute, end_minute FROM ustadz_availability_slots \
         WHERE ustadz_id = ? AND weekday = ? ORDER BY start_minute")
        .bind(ustadz_id).bind(weekday)
        .fetch_all(pool).await.map_err(dberr)?;
    if slots.is_empty() { return Ok((vec![], vec![])); }

    let busy: Vec<(chrono::NaiveDateTime, i64)> = sqlx::query_as(
        "SELECT CAST(scheduled_at AS DATETIME), duration_hours * 60 AS duration_minutes FROM ustadz_visits \
         WHERE ustadz_id = ? AND status IN ('REQUESTED','WAITING_CONFIRM','CONFIRMED')")
        .bind(ustadz_id).fetch_all(pool).await.map_err(dberr)?;

    let now_utc: chrono::NaiveDateTime = sqlx::query_scalar("SELECT UTC_TIMESTAMP()")
        .fetch_one(pool).await.map_err(dberr)?;
    let min_h = setting_i64(pool, "visit_min_schedule_hours", 2).await;
    let earliest_utc = now_utc + chrono::Duration::hours(min_h);
    let max_h_setting = setting_i64(pool, "visit_max_hours", 8).await;

    let mut out = Vec::new();
    let mut max_hours: Vec<(String, i64)> = Vec::new();
    for (start_m, end_m) in slots {
        let mut m = start_m;
        while m < end_m {
            let base_wib = d.and_hms_opt(0, 0, 0).unwrap() + chrono::Duration::minutes(m as i64);
            let start_utc = base_wib - chrono::Duration::hours(7);
            if start_utc >= earliest_utc {
                // jam beruntun maksimal dari jam mulai ini (kena booking / ujung rentang / batas global)
                let mut j: i64 = 0;
                loop {
                    let nxt = j + 1;
                    if nxt > max_h_setting { break; }
                    if m + nxt * 60 > end_m { break; }
                    let e_utc = start_utc + chrono::Duration::hours(nxt as i64);
                    let bentrok = busy.iter().any(|(s, dm)| {
                        let e = *s + chrono::Duration::minutes((*dm).max(1) as i64);
                        *s < e_utc && e > start_utc
                    });
                    if bentrok { break; }
                    j = nxt;
                }
                if j >= 1 {
                    let label = format!("{:02}:{:02}", m / 60, m % 60);
                    max_hours.push((label.clone(), j));
                    if j >= hours { out.push(label); }
                }
            }
            m += 60;
        }
    }
    Ok((out, max_hours))
}

// ===================== create (hold + payment) =====================

pub async fn create_visit(
    state: &AppState,
    user_id: i64,
    req: CreateVisitReq,
    idem_key: &str,
) -> Result<VisitCreatedOut, AppError> {
    if !visit_enabled(&state.pool).await? {
        return Err(AppError::Forbidden("modul Pesan Ustadz sedang nonaktif".into()));
    }
    if !state.visit_create_gate(&user_id) {
        return Err(AppError::RateLimited);
    }
    // idempotency: pre-SELECT
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM ustadz_visits WHERE user_id = ? AND client_key = ?")
        .bind(user_id).bind(idem_key)
        .fetch_optional(&state.pool).await.map_err(dberr)?;
    if let Some(vid) = existing {
        let v = fetch_visit(&state.pool, vid).await?.ok_or_else(|| AppError::NotFound("booking hilang".into()))?;
        let out = visit_out(&state.pool, &v, Some(user_id)).await?;
        return Ok(VisitCreatedOut { visit: out, invoice_url: None, replay: true });
    }

    // ustadz valid + accepting + tarif
    let tarif: Option<(i64,)> = sqlx::query_as(
        "SELECT vs.price_per_hour FROM users u \
         JOIN ustadz_profiles up ON up.user_id = u.id AND up.verified_at IS NOT NULL \
         JOIN ustadz_visit_settings vs ON vs.ustadz_id = u.id AND vs.is_accepting = 1 \
         WHERE u.id = ? AND u.status = 'ACTIVE' AND vs.price_per_hour >= 1000")
        .bind(req.ustadz_id).fetch_optional(&state.pool).await.map_err(dberr)?;
    let price_per_hour = match tarif {
        Some((p,)) => p,
        None => return Err(AppError::Unprocessable("ustadz sedang tidak menerima pesanan".into())),
    };
    if !(1..=8).contains(&req.duration_hours) {
        return Err(AppError::Unprocessable("durasi 1-8 jam".into()));
    }
    let sched_wib = chrono::NaiveDateTime::parse_from_str(
        &format!("{} {}:00", req.date, req.start_time), "%Y-%m-%d %H:%M:%S")
        .map_err(|_| AppError::Unprocessable("tanggal/jam tidak valid".into()))?;
    let sched_utc = sched_wib - chrono::Duration::hours(7);
    let hhmm = format!("{:02}:{:02}", sched_wib.hour(), sched_wib.minute());
    let (avail, _max_hours_chk) = slots_for_date(&state.pool, req.ustadz_id, &req.date, req.duration_hours).await?;
    if !avail.contains(&hhmm) {
        return Err(AppError::Unprocessable(format!("jam {hhmm} tidak tersedia — pilih jam dari daftar")));
    }
    // titik kunjungan: bawaan request; kalau kosong pakai titik rumah santri
    let (visit_lat, visit_lng, visit_label) = match (req.lat, req.lng) {
        (Some(la), Some(ln)) => (la, ln, req.address_label.clone().or(Some("Titik kunjungan".into()))),
        _ => {
            let hp: Option<(f64, f64, Option<String>)> = sqlx::query_as(
                "SELECT CAST(lat AS DOUBLE), CAST(lng AS DOUBLE), address_label FROM user_home_points WHERE user_id = ?")
                .bind(user_id).fetch_optional(&state.pool).await.map_err(dberr)?;
            match hp {
                Some((la, ln, lbl)) => (la, ln, Some(lbl.unwrap_or_else(|| "Titik rumah".into()))),
                None => return Err(AppError::Unprocessable(
                    "set titik rumah Anda dulu (Profil > Data Saya > Titik Rumah)".into())),
            }
        }
    };
    let addr_label: String = req.address_label.as_deref().map(str::trim).filter(|s| !s.is_empty())
        .map(str::to_string)
        .or(visit_label)
        .unwrap_or_default();
    let radius = setting_i64(&state.pool, "visit_radius_km", 5).await.clamp(1, 50) as f64;
    if let Some((ulat, ulng)) = sqlx::query_as::<_, (f64, f64)>(
        "SELECT CAST(point_lat AS DOUBLE), CAST(point_lng AS DOUBLE) FROM ustadz_profiles WHERE user_id = ? AND point_lat IS NOT NULL")
        .bind(req.ustadz_id).fetch_optional(&state.pool).await.map_err(dberr)?
    {
        let dist = haversine_km(visit_lat, visit_lng, ulat, ulng);
        if dist > radius {
            return Err(AppError::Unprocessable(format!(
                "lokasi Anda di luar radius layanan ustadz ({:.1} km > {radius} km)", dist.ceil())));
        }
    }
    let price_total = price_per_hour * req.duration_hours;

    // atomik: lock settings ustadz (serialisasi per ustadz), cek overlap, insert
    let mut tx = state.pool.begin().await.map_err(dberr)?;
    sqlx::query("SELECT ustadz_id FROM ustadz_visit_settings WHERE ustadz_id = ? FOR UPDATE")
        .bind(req.ustadz_id)
        .fetch_optional(&mut *tx).await.map_err(dberr)?;
    let overlap: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ustadz_visits WHERE ustadz_id = ? \
         AND status IN ('REQUESTED','WAITING_CONFIRM','CONFIRMED') \
         AND scheduled_at < DATE_ADD(?, INTERVAL ? MINUTE) \
         AND DATE_ADD(scheduled_at, INTERVAL duration_hours HOUR) > ?")
        .bind(req.ustadz_id).bind(sched_utc).bind(req.duration_hours * 60).bind(sched_utc)
        .fetch_one(&mut *tx).await.map_err(dberr)?;
    if overlap > 0 {
        tx.rollback().await.map_err(dberr)?;
        return Err(AppError::Conflict("ustadz_schedule_conflict: jam itu baru saja terisi".into()));
    }
    let ins = sqlx::query(
        "INSERT INTO ustadz_visits (user_id, ustadz_id, scheduled_at, duration_hours, \
         lat, lng, address_label, note, client_key, price_per_hour, price_total, status, hold_expires_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'REQUESTED', \
         DATE_ADD(UTC_TIMESTAMP(), INTERVAL (SELECT CAST(value AS UNSIGNED) FROM settings WHERE `key`='visit_invoice_duration_sec') SECOND))")
        .bind(user_id).bind(req.ustadz_id).bind(sched_utc).bind(req.duration_hours)
        .bind(visit_lat).bind(visit_lng).bind(addr_label)
        .bind(req.note.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .bind(idem_key).bind(price_per_hour).bind(price_total)
        .execute(&mut *tx).await;
    let visit_id = match ins {
        Ok(r) => r.last_insert_id() as i64,
        Err(e) => {
            let m = format!("{e}");
            // 0023: uv_active_uq (max 1 pesanan aktif per santri) sudah dihapus — santri boleh beberapa pesanan.
            if m.contains("uv_client_uq") {
                let vid: (i64,) = sqlx::query_as("SELECT id FROM ustadz_visits WHERE user_id = ? AND client_key = ?")
                    .bind(user_id).bind(idem_key).fetch_one(&state.pool).await.map_err(dberr)?;
                let v = fetch_visit(&state.pool, vid.0).await?.unwrap();
                return Ok(VisitCreatedOut { visit: visit_out(&state.pool, &v, Some(user_id)).await?, invoice_url: None, replay: true });
            }
            return Err(dberr(e));
        }
    };
    log_history(&mut *tx, visit_id, None, "REQUESTED", Some(user_id), "booking dibuat (slot ditahan)").await?;
    tx.commit().await.map_err(dberr)?;

    let (payment_id, external_id) = payments::create_payment_pending(state, visit_id, price_total).await?;
    let invoice_url = payments::try_issue_invoice(state, payment_id, &external_id, price_total).await;

    let v = fetch_visit(&state.pool, visit_id).await?.unwrap();
    Ok(VisitCreatedOut {
        visit: visit_out(&state.pool, &v, Some(user_id)).await?,
        invoice_url,
        replay: false,
    })
}

pub async fn list_my(pool: &MySqlPool, user_id: i64, limit: i64, cursor: Option<i64>) -> Result<(Vec<VisitOut>, Option<String>), AppError> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM ustadz_visits WHERE user_id = ? AND (? IS NULL OR id < ?) ORDER BY id DESC LIMIT ?")
        .bind(user_id).bind(cursor).bind(cursor).bind(limit + 1)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    let mut next = None;
    for (i, (id,)) in rows.into_iter().enumerate() {
        if (i as i64) == limit { next = Some(id.to_string()); break; }
        if let Some(v) = fetch_visit(pool, id).await? {
            out.push(visit_out(pool, &v, Some(user_id)).await?);
        }
    }
    Ok((out, next))
}

pub async fn require_visit_access(pool: &MySqlPool, user: &crate::middleware::auth::CurrentUser, visit_id: i64) -> Result<VisitRow, AppError> {
    let v = fetch_visit(pool, visit_id).await?.ok_or_else(|| AppError::NotFound("pesanan tidak ada".into()))?;
    let is_party = v.user_id == user.user_id || v.ustadz_id == user.user_id;
    if !is_party && !user.permissions.contains("visits.admin") {
        return Err(AppError::NotFound("pesanan tidak ada".into()));
    }
    Ok(v)
}

// ===================== cancel =====================

pub async fn cancel(state: &AppState, user: &crate::middleware::auth::CurrentUser, visit_id: i64) -> Result<VisitOut, AppError> {
    let v = require_visit_access(&state.pool, user, visit_id).await?;
    if v.user_id != user.user_id {
        return Err(AppError::Forbidden("hanya pemesan yang bisa membatalkan".into()));
    }
    match v.status.as_str() {
        "REQUESTED" => {
            let mut tx = state.pool.begin().await.map_err(dberr)?;
            sqlx::query("UPDATE payments SET status = 'EXPIRED' WHERE subject_type = 'ustadz_visit' AND subject_id = ? AND status = 'PENDING'")
                .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?;
            sqlx::query("UPDATE ustadz_visits SET status = 'CANCELED', canceled_at = UTC_TIMESTAMP(), canceled_by = ?, cancel_reason = 'dibatalkan sebelum pembayaran'")
                .bind(user.user_id).execute(&mut *tx).await.map_err(dberr)?;
            log_history(&mut *tx, visit_id, Some("REQUESTED"), "CANCELED", Some(user.user_id), "cancel sebelum bayar").await?;
            tx.commit().await.map_err(dberr)?;
        }
        "WAITING_CONFIRM" => {
            // Aturan 16Sep: setelah ustadz ACC (CONFIRMED) santri TIDAK bisa membatalkan.
            // WAITING_CONFIRM (belum di-ACC) masih boleh — dana kembali penuh.
            let mut tx = state.pool.begin().await.map_err(dberr)?;
            sqlx::query("UPDATE ustadz_visits SET status = 'CANCELED', canceled_at = UTC_TIMESTAMP(), canceled_by = ?, cancel_reason = ?")
                .bind(user.user_id)
                .bind("dibatalkan sebelum dikonfirmasi — dana dikembalikan ke saldo")
                .execute(&mut *tx).await.map_err(dberr)?;
            log_history(&mut *tx, visit_id, Some(v.status.as_str()), "CANCELED", Some(user.user_id), "refund penuh ke saldo (belum di-ACC)").await?;
            tx.commit().await.map_err(dberr)?;
            payments::refund_visit_to_deposit(state, visit_id, "santri cancel").await?;
            let v2 = fetch_visit(&state.pool, visit_id).await?.unwrap();
            notify(&state.pool, v2.ustadz_id, "VISIT_CANCELED", "Kunjungan dibatalkan",
                &format!("Kunjungan {} dibatalkan oleh santri (sebelum konfirmasi).", v2.scheduled_at), &visit_id.to_string()).await?;
        }
        "CONFIRMED" => {
            return Err(AppError::Conflict(
                "Kunjungan sudah dikonfirmasi ustadz — tidak bisa dibatalkan santri. Hubungi admin pondok bila darurat.".into()));
        }
        other => return Err(AppError::Unprocessable(format!("status {other} tidak bisa dibatalkan"))),
    }
    let v2 = fetch_visit(&state.pool, visit_id).await?.unwrap();
    Ok(visit_out(&state.pool, &v2, Some(user.user_id)).await?)
}

// ===================== chat =====================

pub async fn messages_list(
    pool: &MySqlPool,
    user: &crate::middleware::auth::CurrentUser,
    visit_id: i64,
    limit: i64,
    cursor: Option<i64>,
) -> Result<(Vec<MessageOut>, Option<String>), AppError> {
    let v = require_visit_access(pool, user, visit_id).await?;
    let rows: Vec<(i64, i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, sender_id, body, DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(read_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM visit_messages WHERE visit_id = ? AND (? IS NULL OR id < ?) ORDER BY id DESC LIMIT ?")
        .bind(visit_id).bind(cursor).bind(cursor).bind(limit + 1)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    let mut next = None;
    for (i, r) in rows.into_iter().enumerate() {
        if (i as i64) == limit { next = Some(r.0.to_string()); break; }
        out.push(MessageOut { id: r.0, sender_id: r.1, body: r.2, created_at: r.3, read_at: r.4 });
    }
    out.reverse();
    sqlx::query("UPDATE visit_messages SET read_at = UTC_TIMESTAMP() WHERE visit_id = ? AND sender_id != ? AND read_at IS NULL")
        .bind(visit_id).bind(user.user_id)
        .execute(pool).await.map_err(dberr)?;
    Ok((out, next))
}

pub async fn messages_send(
    state: &AppState,
    user: &crate::middleware::auth::CurrentUser,
    visit_id: i64,
    body: &str,
) -> Result<MessageOut, AppError> {
    let v = require_visit_access(&state.pool, user, visit_id).await?;
    if v.status != "CONFIRMED" {
        return Err(AppError::Forbidden("chat aktif hanya saat kunjungan dikonfirmasi".into()));
    }
    if !state.visit_chat_gate(&user.user_id) {
        return Err(AppError::RateLimited);
    }
    let body = body.trim();
    if body.is_empty() || body.len() > 1000 {
        return Err(AppError::Unprocessable("pesan 1-1000 karakter".into()));
    }
    let ins = sqlx::query("INSERT INTO visit_messages (visit_id, sender_id, body) VALUES (?, ?, ?)")
        .bind(visit_id).bind(user.user_id).bind(body)
        .execute(&state.pool).await.map_err(dberr)?;
    Ok(MessageOut {
        id: ins.last_insert_id() as i64,
        sender_id: user.user_id,
        body: body.to_string(),
        created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        read_at: None,
    })
}

pub async fn unread_count(pool: &MySqlPool, user_id: i64, visit_id: i64) -> Result<i64, AppError> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM visit_messages vm JOIN ustadz_visits v ON v.id = vm.visit_id \
         WHERE vm.visit_id = ? AND vm.sender_id != ? AND vm.read_at IS NULL \
           AND (v.user_id = ? OR v.ustadz_id = ?)")
        .bind(visit_id).bind(user_id).bind(user_id).bind(user_id)
        .fetch_one(pool).await.map_err(dberr)?;
    Ok(n)
}

// ===================== review =====================

pub async fn submit_review(
    pool: &MySqlPool,
    user_id: i64,
    visit_id: i64,
    rating: i8,
    comment: Option<String>,
) -> Result<(), AppError> {
    let v = fetch_visit(pool, visit_id).await?.ok_or_else(|| AppError::NotFound("pesanan tidak ada".into()))?;
    if v.status != "COMPLETED" {
        return Err(AppError::Unprocessable("review hanya setelah kunjungan selesai (COMPLETED)".into()));
    }
    if !(1..=5).contains(&rating) {
        return Err(AppError::Unprocessable("rating 1-5".into()));
    }
    let (direction, reviewer, reviewee) = if user_id == v.user_id {
        ("SANTRI_TO_USTADZ", v.user_id, v.ustadz_id)
    } else if user_id == v.ustadz_id {
        ("USTADZ_TO_SANTRI", v.ustadz_id, v.user_id)
    } else {
        return Err(AppError::NotFound("pesanan tidak ada".into()));
    };
    let ins = sqlx::query(
        "INSERT INTO visit_reviews (visit_id, direction, reviewer_id, reviewee_id, rating, comment) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(visit_id).bind(direction).bind(reviewer).bind(reviewee).bind(rating)
        .bind(comment.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .execute(pool).await;
    if let Err(e) = ins {
        let m = format!("{e}");
        if m.contains("vr_unique_dir") {
            return Err(AppError::Conflict("sudah memberi review untuk pesanan ini".into()));
        }
        return Err(dberr(e));
    }
    let both: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM visit_reviews WHERE visit_id = ?")
        .bind(visit_id).fetch_one(pool).await.map_err(dberr)?;
    if both >= 2 {
        let mut tx = pool.begin().await.map_err(dberr)?;
        sqlx::query("UPDATE visit_reviews SET revealed_at = UTC_TIMESTAMP() WHERE visit_id = ? AND revealed_at IS NULL")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?;
        sqlx::query("UPDATE ustadz_visits SET status = 'REVIEWED', reviewed_at = UTC_TIMESTAMP() WHERE id = ? AND status = 'COMPLETED'")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?;
        log_history(&mut *tx, visit_id, Some("COMPLETED"), "REVIEWED", Some(user_id), "kedua review masuk — reveal").await?;
        tx.commit().await.map_err(dberr)?;
    }
    Ok(())
}

pub async fn review_status(pool: &MySqlPool, user_id: i64, visit_id: i64) -> Result<ReviewStatusOut, AppError> {
    let v = fetch_visit(pool, visit_id).await?.ok_or_else(|| AppError::NotFound("pesanan tidak ada".into()))?;
    if v.user_id != user_id && v.ustadz_id != user_id {
        return Err(AppError::NotFound("pesanan tidak ada".into()));
    }
    let mine: Option<(i8, Option<String>, i8)> = sqlx::query_as(
        "SELECT rating, comment, revealed_at IS NOT NULL FROM visit_reviews WHERE visit_id = ? AND reviewer_id = ?")
        .bind(visit_id).bind(user_id).fetch_optional(pool).await.map_err(dberr)?;
    let counterpart: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM visit_reviews WHERE visit_id = ? AND reviewer_id != ?")
        .bind(visit_id).bind(user_id).fetch_one(pool).await.map_err(dberr)?;
    let revealed = mine.as_ref().map(|m| m.2 != 0).unwrap_or(v.status == "REVIEWED");
    // rating & komentar lawan — hanya setelah reveal (jaga double-blind)
    let (cp_rating, cp_comment): (Option<i8>, Option<String>) = if revealed {
        sqlx::query_as("SELECT rating, comment FROM visit_reviews WHERE visit_id = ? AND reviewer_id != ?")
            .bind(visit_id).bind(user_id)
            .fetch_optional(pool).await.map_err(dberr)?
            .map(|(r, c)| (Some(r), c)).unwrap_or((None, None))
    } else {
        (None, None)
    };
    Ok(ReviewStatusOut {
        can_review: v.status == "COMPLETED" && mine.is_none(),
        my_rating: mine.as_ref().map(|m| m.0),
        my_comment: mine.as_ref().and_then(|m| m.1.clone()),
        counterpart_submitted: counterpart > 0,
        revealed,
        counterpart_rating: cp_rating,
        counterpart_comment: cp_comment,
    })
}

pub async fn ustadz_public_reviews(pool: &MySqlPool, ustadz_id: i64, limit: i64, cursor: Option<i64>) -> Result<(Vec<ReviewPublicOut>, Option<String>), AppError> {
    let rows: Vec<(i64, Option<String>, i8, Option<String>, String)> = sqlx::query_as(
        "SELECT vr.id, COALESCE(NULLIF(up.full_name,''),'Santri'), vr.rating, vr.comment, DATE_FORMAT(vr.created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM visit_reviews vr \
         JOIN users u ON u.id = vr.reviewer_id \
         LEFT JOIN user_profiles up ON up.user_id = u.id \
         WHERE vr.reviewee_id = ? AND vr.direction = 'SANTRI_TO_USTADZ' \
           AND vr.revealed_at IS NOT NULL AND vr.hidden = 0 \
           AND (? IS NULL OR vr.id < ?) ORDER BY vr.id DESC LIMIT ?")
        .bind(ustadz_id).bind(cursor).bind(cursor).bind(limit + 1)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    let mut next = None;
    for (i, r) in rows.into_iter().enumerate() {
        if (i as i64) == limit { next = Some(r.0.to_string()); break; }
        out.push(ReviewPublicOut {
            id: r.0,
            reviewer_first_name: r.1.unwrap_or_else(|| "Santri".into()).split(' ').next().unwrap_or("Santri").to_string(),
            rating: r.2, comment: r.3, created_at: r.4,
        });
    }
    Ok((out, next))
}

pub async fn requester_reviews(
    pool: &MySqlPool,
    ustadz_user_id: i64,
    visit_id: i64,
    limit: i64,
    cursor: Option<i64>,
) -> Result<(Vec<ReviewPublicOut>, Option<String>), AppError> {
    let v = fetch_visit(pool, visit_id).await?.ok_or_else(|| AppError::NotFound("pesanan tidak ada".into()))?;
    if v.ustadz_id != ustadz_user_id {
        return Err(AppError::NotFound("pesanan tidak ada".into()));
    }
    let rows: Vec<(i64, Option<String>, i8, Option<String>, String)> = sqlx::query_as(
        "SELECT vr.id, COALESCE(NULLIF(up.full_name,''),'Ustadz'), vr.rating, vr.comment, DATE_FORMAT(vr.created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM visit_reviews vr \
         LEFT JOIN user_profiles up ON up.user_id = vr.reviewer_id \
         WHERE vr.reviewee_id = ? AND vr.direction = 'USTADZ_TO_SANTRI' \
           AND vr.revealed_at IS NOT NULL AND vr.hidden = 0 \
           AND (? IS NULL OR vr.id < ?) ORDER BY vr.id DESC LIMIT ?")
        .bind(v.user_id).bind(cursor).bind(cursor).bind(limit + 1)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    let mut next = None;
    for (i, r) in rows.into_iter().enumerate() {
        if (i as i64) == limit { next = Some(r.0.to_string()); break; }
        out.push(ReviewPublicOut {
            id: r.0,
            reviewer_first_name: r.1.unwrap_or_else(|| "Ustadz".into()).split(' ').next().unwrap_or("Ustadz").to_string(),
            rating: r.2, comment: r.3, created_at: r.4,
        });
    }
    Ok((out, next))
}

// ===================== lokasi =====================

pub async fn put_my_location(state: &AppState, user_id: i64, lat: f64, lng: f64, acc: Option<i16>) -> Result<(), AppError> {
    if !state.visit_location_gate(&user_id) {
        return Err(AppError::RateLimited);
    }
    sqlx::query(
        "INSERT INTO user_locations (user_id, lat, lng, accuracy_m, recorded_at) VALUES (?, ?, ?, ?, UTC_TIMESTAMP()) AS new \
         ON DUPLICATE KEY UPDATE lat = new.lat, lng = new.lng, accuracy_m = new.accuracy_m, recorded_at = UTC_TIMESTAMP()")
        .bind(user_id).bind(lat).bind(lng).bind(acc)
        .execute(&state.pool).await.map_err(dberr)?;
    Ok(())
}

// ===================== ustadz =====================

pub async fn get_visit_settings(pool: &MySqlPool, ustadz_id: i64) -> Result<VisitSettingsOut, AppError> {
    let row: Option<(i8, i64, i64)> = sqlx::query_as(
        "SELECT is_accepting, max_active_visits, price_per_hour FROM ustadz_visit_settings WHERE ustadz_id = ?")
        .bind(ustadz_id).fetch_optional(pool).await.map_err(dberr)?;
    Ok(match row {
        Some((a, m, p)) => VisitSettingsOut { is_accepting: a != 0, max_active_visits: m, price_per_hour: p },
        None => VisitSettingsOut { is_accepting: false, max_active_visits: 2, price_per_hour: MIN_TARIF_PER_JAM },
    })
}

pub async fn put_visit_settings(pool: &MySqlPool, ustadz_id: i64, req: VisitSettingsReq) -> Result<VisitSettingsOut, AppError> {
    if !(1..=10).contains(&req.max_active_visits) {
        return Err(AppError::Unprocessable("max_active_visits 1-10".into()));
    }
    if req.price_per_hour < MIN_TARIF_PER_JAM || req.price_per_hour > 100_000_000 {
        return Err(AppError::Unprocessable(format!("tarif per jam Rp {MIN_TARIF_PER_JAM} - Rp 100.000.000")));
    }
    sqlx::query(
        "INSERT INTO ustadz_visit_settings (ustadz_id, is_accepting, max_active_visits, price_per_hour) \
         VALUES (?, ?, ?, ?) AS new \
         ON DUPLICATE KEY UPDATE is_accepting = new.is_accepting, max_active_visits = new.max_active_visits, price_per_hour = new.price_per_hour")
        .bind(ustadz_id).bind(req.is_accepting as i8).bind(req.max_active_visits).bind(req.price_per_hour)
        .execute(pool).await.map_err(dberr)?;
    Ok(VisitSettingsOut { is_accepting: req.is_accepting, max_active_visits: req.max_active_visits, price_per_hour: req.price_per_hour })
}

pub async fn my_visits(pool: &MySqlPool, ustadz_id: i64) -> Result<MyVisitsOut, AppError> {
    let rows: Vec<(i64, String, String, i64, i64, Option<String>, String, i64, String, Option<f64>, i64)> = sqlx::query_as(
        "SELECT v.id, v.status, DATE_FORMAT(v.scheduled_at, '%Y-%m-%dT%H:%i:%sZ'), v.duration_hours, v.price_total, v.note, \
         v.address_label, v.user_id, COALESCE(NULLIF(up.full_name,''),'Santri'), \
         (SELECT CAST(AVG(vr.rating) AS DOUBLE) FROM visit_reviews vr WHERE vr.reviewee_id = v.user_id \
            AND vr.direction = 'USTADZ_TO_SANTRI' AND vr.revealed_at IS NOT NULL AND vr.hidden = 0), \
         (SELECT COUNT(*) FROM visit_reviews vr2 WHERE vr2.reviewee_id = v.user_id \
            AND vr2.direction = 'USTADZ_TO_SANTRI' AND vr2.revealed_at IS NOT NULL AND vr2.hidden = 0) \
         FROM ustadz_visits v \
         LEFT JOIN user_profiles up ON up.user_id = v.user_id \
         WHERE v.ustadz_id = ? AND v.status = 'WAITING_CONFIRM' ORDER BY v.scheduled_at ASC LIMIT 50")
        .bind(ustadz_id).fetch_all(pool).await.map_err(dberr)?;
    let incoming = rows.into_iter().map(|r| IncomingVisitOut {
        id: r.0, status: r.1, scheduled_at: r.2, duration_hours: r.3, price_total: r.4,
        note: r.5, address_label: r.6,
        requester: RequesterOut { user_id: r.7, full_name: r.8, rating_avg: r.9, rating_count: r.10 },
    }).collect();
    let rows2: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM ustadz_visits WHERE ustadz_id = ? AND status = 'CONFIRMED' ORDER BY scheduled_at ASC LIMIT 50")
        .bind(ustadz_id).fetch_all(pool).await.map_err(dberr)?;
    let mut upcoming = Vec::new();
    for (id,) in rows2 {
        if let Some(v) = fetch_visit(pool, id).await? {
            upcoming.push(visit_out(pool, &v, Some(ustadz_id)).await?);
        }
    }
    // riwayat: status terminal, terbaru dulu
    let rows3: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM ustadz_visits WHERE ustadz_id = ? \
         AND status IN ('COMPLETED','REVIEWED','CANCELED','DECLINED','PAYMENT_EXPIRED') \
         ORDER BY COALESCE(reviewed_at, completed_at, canceled_at, declined_at, created_at) DESC LIMIT 30")
        .bind(ustadz_id).fetch_all(pool).await.map_err(dberr)?;
    let mut history = Vec::new();
    for (id,) in rows3 {
        if let Some(v) = fetch_visit(pool, id).await? {
            history.push(visit_out(pool, &v, Some(ustadz_id)).await?);
        }
    }
    Ok(MyVisitsOut { incoming, upcoming, history })
}

pub async fn confirm_visit(state: &AppState, ustadz_user_id: i64, visit_id: i64) -> Result<VisitOut, AppError> {
    let mut tx = state.pool.begin().await.map_err(dberr)?;
    sqlx::query("SELECT ustadz_id FROM ustadz_visit_settings WHERE ustadz_id = ? FOR UPDATE")
        .bind(ustadz_user_id)
        .fetch_optional(&mut *tx).await.map_err(dberr)?;
    let v = fetch_visit_tx(&mut tx, visit_id).await?.ok_or_else(|| AppError::NotFound("pesanan tidak ada".into()))?;
    if v.ustadz_id != ustadz_user_id {
        return Err(AppError::NotFound("pesanan tidak ada".into()));
    }
    if v.status != "WAITING_CONFIRM" {
        return Err(AppError::Conflict(format!("status {}, harus WAITING_CONFIRM", v.status)));
    }
    let overlap: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ustadz_visits WHERE ustadz_id = ? AND status = 'CONFIRMED' AND id != ? \
         AND scheduled_at < DATE_ADD(?, INTERVAL ? MINUTE) \
         AND DATE_ADD(scheduled_at, INTERVAL duration_hours HOUR) > ?")
        .bind(ustadz_user_id).bind(visit_id).bind(&v.scheduled_at).bind(v.duration_hours * 60).bind(&v.scheduled_at)
        .fetch_one(&mut *tx).await.map_err(dberr)?;
    if overlap > 0 {
        tx.rollback().await.map_err(dberr)?;
        return Err(AppError::Conflict("ustadz_schedule_conflict: bentrok jadwal dgn kunjungan lain".into()));
    }
    let n = sqlx::query("UPDATE ustadz_visits SET status = 'CONFIRMED', confirmed_at = UTC_TIMESTAMP() WHERE id = ? AND status = 'WAITING_CONFIRM'")
        .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Err(AppError::Conflict("pesanan sudah tidak menunggu konfirmasi".into()));
    }
    log_history(&mut *tx, visit_id, Some("WAITING_CONFIRM"), "CONFIRMED", Some(ustadz_user_id), "ustadz mengonfirmasi").await?;
    notify(&mut *tx, v.user_id, "KHATMIL_ASSIGN_RESULT", "Kunjungan dikonfirmasi",
        &format!("Ustadz telah MENGONFIRMASI kunjungan {}. Kontak & chat kini terbuka.", v.scheduled_at), &visit_id.to_string()).await?;
    tx.commit().await.map_err(dberr)?;
    let v2 = fetch_visit(&state.pool, visit_id).await?.unwrap();
    Ok(visit_out(&state.pool, &v2, Some(ustadz_user_id)).await?)
}


pub async fn decline_visit(state: &AppState, ustadz_user_id: i64, visit_id: i64, reason: &str) -> Result<VisitOut, AppError> {
    let v = fetch_visit(&state.pool, visit_id).await?.ok_or_else(|| AppError::NotFound("pesanan tidak ada".into()))?;
    if v.ustadz_id != ustadz_user_id {
        return Err(AppError::NotFound("pesanan tidak ada".into()));
    }
    if v.status != "WAITING_CONFIRM" {
        return Err(AppError::Conflict(format!("status {}, harus WAITING_CONFIRM", v.status)));
    }
    let reason = reason.trim();
    let mut tx = state.pool.begin().await.map_err(dberr)?;
    sqlx::query("UPDATE ustadz_visits SET status = 'DECLINED', declined_at = UTC_TIMESTAMP(), decline_reason = ? WHERE id = ? AND status = 'WAITING_CONFIRM'")
        .bind(if reason.is_empty() { "—" } else { reason }).bind(visit_id)
        .execute(&mut *tx).await.map_err(dberr)?;
    log_history(&mut *tx, visit_id, Some("WAITING_CONFIRM"), "DECLINED", Some(ustadz_user_id), "ustadz menolak — refund ke saldo santri").await?;
    tx.commit().await.map_err(dberr)?;
    payments::refund_visit_to_deposit(state, visit_id, "ustadz menolak").await?;
    notify(&state.pool, v.user_id, "KHATMIL_ASSIGN_RESULT", "Permintaan ditolak ustadz",
        &format!("Ustadz menolak kunjungan {}. Dana PENUH dikembalikan ke saldo Anda.", v.scheduled_at), &visit_id.to_string()).await?;
    let v2 = fetch_visit(&state.pool, visit_id).await?.unwrap();
    Ok(visit_out(&state.pool, &v2, Some(ustadz_user_id)).await?)
}

pub async fn complete_visit(state: &AppState, ustadz_user_id: i64, visit_id: i64) -> Result<VisitOut, AppError> {
    let v = fetch_visit(&state.pool, visit_id).await?.ok_or_else(|| AppError::NotFound("pesanan tidak ada".into()))?;
    if v.ustadz_id != ustadz_user_id {
        return Err(AppError::NotFound("pesanan tidak ada".into()));
    }
    if v.status != "CONFIRMED" {
        return Err(AppError::Conflict(format!("status {}, harus CONFIRMED", v.status)));
    }
    // Aturan 16Sep: ustadz hanya bisa menandai selesai SETELAH waktu kunjungan berakhir
    // (scheduled_at + duration_hours). Batas atas tetap 24 jam setelahnya. Sebelum itu -> admin force.
    let in_window: i64 = sqlx::query_scalar(
        "SELECT CASE WHEN UTC_TIMESTAMP() >= DATE_ADD(?, INTERVAL duration_hours HOUR) \
              AND UTC_TIMESTAMP() <= DATE_ADD(DATE_ADD(?, INTERVAL duration_hours HOUR), INTERVAL 24 HOUR) \
         THEN 1 ELSE 0 END FROM ustadz_visits WHERE id = ?")
        .bind(&v.scheduled_at).bind(&v.scheduled_at).bind(visit_id)
        .fetch_one(&state.pool).await.map_err(dberr)?;
    if in_window == 0 {
        return Err(AppError::Unprocessable("belum/kurang dari jadwal — kunjungan hanya bisa ditandai selesai setelah waktunya berakhir".into()));
    }
    let mut tx = state.pool.begin().await.map_err(dberr)?;
    sqlx::query("UPDATE ustadz_visits SET status = 'COMPLETED', completed_at = UTC_TIMESTAMP() WHERE id = ? AND status = 'CONFIRMED'")
        .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?;
    log_history(&mut *tx, visit_id, Some("CONFIRMED"), "COMPLETED", Some(ustadz_user_id), "kunjungan selesai").await?;
    notify(&mut *tx, v.user_id, "KHATMIL_ASSIGN_RESULT", "Kunjungan selesai",
        &format!("Alhamdulillah, kunjungan {} selesai. Beri rating untuk ustadz Anda.", v.scheduled_at), &visit_id.to_string()).await?;
    notify(&mut *tx, ustadz_user_id, "KHATMIL_ASSIGN_RESULT", "Nilai santri Anda",
        &format!("Kunjungan {} selesai. Beri rating & catatan utk santri (privat).", v.scheduled_at), &visit_id.to_string()).await?;
    tx.commit().await.map_err(dberr)?;
    // penghasilan ustadz masuk saldo otomatis
    payments::earn_visit_income(state, visit_id, v.ustadz_id).await?;
    let v2 = fetch_visit(&state.pool, visit_id).await?.unwrap();
    Ok(visit_out(&state.pool, &v2, Some(ustadz_user_id)).await?)
}

// ===================== admin =====================

pub async fn admin_list_visits(pool: &MySqlPool, f: &AdminVisitFilter, limit: i64) -> Result<(Vec<VisitOut>, Option<String>), AppError> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM ustadz_visits \
         WHERE (? IS NULL OR status = ?) AND (? IS NULL OR ustadz_id = ?) AND (? IS NULL OR user_id = ?) \
           AND (? IS NULL OR id < ?) ORDER BY id DESC LIMIT ?")
        .bind(&f.status).bind(&f.status)
        .bind(f.ustadz_id).bind(f.ustadz_id)
        .bind(f.user_id).bind(f.user_id)
        .bind(f.cursor).bind(f.cursor).bind(limit + 1)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    let mut next = None;
    for (i, (id,)) in rows.into_iter().enumerate() {
        if (i as i64) == limit { next = Some(id.to_string()); break; }
        if let Some(v) = fetch_visit(pool, id).await? {
            out.push(visit_out(pool, &v, None).await?);
        }
    }
    Ok((out, next))
}

pub async fn admin_force_complete(state: &AppState, admin_id: i64, visit_id: i64) -> Result<VisitOut, AppError> {
    let v = fetch_visit(&state.pool, visit_id).await?.ok_or_else(|| AppError::NotFound("pesanan tidak ada".into()))?;
    if !matches!(v.status.as_str(), "CONFIRMED" | "COMPLETED" | "REVIEWED") {
        return Err(AppError::Unprocessable(format!("status {} tidak bisa dipaksa selesai", v.status)));
    }
    if v.status == "CONFIRMED" {
        let mut tx = state.pool.begin().await.map_err(dberr)?;
        sqlx::query("UPDATE ustadz_visits SET status = 'COMPLETED', completed_at = UTC_TIMESTAMP() WHERE id = ? AND status = 'CONFIRMED'")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?;
        log_history(&mut *tx, visit_id, Some("CONFIRMED"), "COMPLETED", Some(admin_id), "ADMIN force-complete").await?;
        notify(&mut *tx, v.user_id, "KHATMIL_ASSIGN_RESULT", "Kunjungan selesai (admin)",
            &format!("Kunjungan {} ditandai selesai oleh admin.", v.scheduled_at), &visit_id.to_string()).await?;
        tx.commit().await.map_err(dberr)?;
        payments::earn_visit_income(state, visit_id, v.ustadz_id).await?;
    }
    let v2 = fetch_visit(&state.pool, visit_id).await?.unwrap();
    Ok(visit_out(&state.pool, &v2, None).await?)
}

pub async fn admin_force_cancel(state: &AppState, admin_id: i64, visit_id: i64, req: ForceCancelReq) -> Result<VisitOut, AppError> {
    let v = fetch_visit(&state.pool, visit_id).await?.ok_or_else(|| AppError::NotFound("pesanan tidak ada".into()))?;
    if matches!(v.status.as_str(), "CANCELED" | "DECLINED" | "PAYMENT_EXPIRED" | "REVIEWED") {
        return Err(AppError::Unprocessable(format!("status {} terminal", v.status)));
    }
    let paid = matches!(v.status.as_str(), "WAITING_CONFIRM" | "CONFIRMED" | "COMPLETED");
    let mut tx = state.pool.begin().await.map_err(dberr)?;
    sqlx::query("UPDATE ustadz_visits SET status = 'CANCELED', canceled_at = UTC_TIMESTAMP(), canceled_by = ?, cancel_reason = ? WHERE id = ?")
        .bind(admin_id).bind(format!("admin: {}", req.reason.trim())).bind(visit_id)
        .execute(&mut *tx).await.map_err(dberr)?;
    log_history(&mut *tx, visit_id, Some(v.status.as_str()), "CANCELED", Some(admin_id), "ADMIN force-cancel").await?;
    tx.commit().await.map_err(dberr)?;
    if paid && req.force_refund {
        payments::refund_visit_to_deposit(state, visit_id, "admin force-cancel").await?;
    }
    let v2 = fetch_visit(&state.pool, visit_id).await?.unwrap();
    Ok(visit_out(&state.pool, &v2, None).await?)
}

pub async fn admin_list_reviews(pool: &MySqlPool, direction: Option<&str>, limit: i64, cursor: Option<i64>) -> Result<(Vec<serde_json::Value>, Option<String>), AppError> {
    let rows: Vec<(i64, String, i64, i64, i8, Option<String>, i8, i8, String)> = sqlx::query_as(
        "SELECT vr.id, vr.direction, vr.reviewer_id, vr.reviewee_id, vr.rating, vr.comment, vr.hidden, vr.revealed_at IS NOT NULL, \
         DATE_FORMAT(vr.created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM visit_reviews vr WHERE (? IS NULL OR vr.direction = ?) AND (? IS NULL OR vr.id < ?) ORDER BY vr.id DESC LIMIT ?")
        .bind(direction).bind(direction).bind(cursor).bind(cursor).bind(limit + 1)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    let mut next = None;
    for (i, r) in rows.into_iter().enumerate() {
        if (i as i64) == limit { next = Some(r.0.to_string()); break; }
        out.push(serde_json::json!({
            "id": r.0, "direction": r.1, "reviewer_id": r.2, "reviewee_id": r.3,
            "rating": r.4, "comment": r.5, "hidden": r.6 != 0, "revealed": r.7 != 0,
            "created_at": r.8,
        }));
    }
    Ok((out, next))
}

pub async fn admin_set_review_hidden(pool: &MySqlPool, review_id: i64, hidden: bool) -> Result<(), AppError> {
    let n = sqlx::query("UPDATE visit_reviews SET hidden = ? WHERE id = ?")
        .bind(hidden as i8).bind(review_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 { return Err(AppError::NotFound("review tidak ada".into())); }
    Ok(())
}
