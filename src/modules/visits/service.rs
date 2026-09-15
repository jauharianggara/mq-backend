//! Service modul visits — core (Bagian V — Pesan Ustadz).
use sqlx::MySqlPool;

use crate::modules::visits::dto::*;
use crate::shared::error::AppError;
use crate::state::AppState;

pub fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}

/// Minimal tarif default (open-decision k plan V; skema absolut Rp 1.000 — V0 finalisasi).
pub const MIN_TARIF: i64 = 10_000;

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

fn parse_iso(s: &str) -> Result<chrono::NaiveDateTime, AppError> {
    let s = s.trim().trim_end_matches('Z');
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
        .map_err(|_| AppError::Unprocessable("scheduled_at format ISO UTC (2026-09-20T14:00:00Z)".into()))
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
         VALUES (?, ?, ?, ?, CAST(? AS JSON), 'IN_APP')",
    )
    .bind(user_id)
    .bind(code)
    .bind(title)
    .bind(body)
    .bind(data)
    .execute(ex)
    .await
    .map_err(dberr)?;
    Ok(())
}

pub async fn log_history<'e, E>(ex: E, visit_id: i64, from: Option<&str>, to: &str, actor: Option<i64>, note: &str) -> Result<(), AppError>
where
    E: sqlx::Executor<'e, Database = sqlx::MySql>,
{
    sqlx::query(
        "INSERT INTO ustadz_visit_status_history (visit_id, from_status, to_status, actor_id, note) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(visit_id)
    .bind(from)
    .bind(to)
    .bind(actor)
    .bind(note)
    .execute(ex)
    .await
    .map_err(dberr)?;
    Ok(())
}

/// Kirim notif ke semua ADMIN/SUPER_ADMIN (mis. refund manual).
pub async fn notify_admins(pool: &MySqlPool, code: &str, title: &str, body: &str, deeplink: &str) {
    let ids: Vec<(i64,)> = sqlx::query_as(
        "SELECT DISTINCT ur.user_id FROM user_roles ur \
         JOIN roles r ON r.id = ur.role_id WHERE r.code IN ('ADMIN','SUPER_ADMIN') LIMIT 20")
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    for (uid,) in ids {
        let _ = notify(&mut *pool.acquire().await.unwrap(), uid, code, title, body, deeplink).await;
    }
}

// ===================== row mappers =====================

pub struct VisitRow {
    pub id: i64,
    pub user_id: i64,
    pub ustadz_id: i64,
    pub service_type_id: i64,
    pub service_name: String,
    pub scheduled_at: String,
    pub duration: i64,
    pub lat: f64,
    pub lng: f64,
    pub address_label: String,
    pub note: Option<String>,
    pub price: i64,
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

#[allow(dead_code)]
const VISIT_COLS: &str = "v.id, v.user_id, v.ustadz_id, v.service_type_id, vst.name, DATE_FORMAT(v.scheduled_at, '%Y-%m-%dT%H:%i:%sZ'), v.duration_minutes, \
     CAST(v.lat AS DOUBLE), CAST(v.lng AS DOUBLE), v.address_label, v.note, v.price_amount, v.anonymized, v.status, \
     v.cancel_reason, v.decline_reason, v.created_at, v.paid_at, v.confirmed_at, v.completed_at, v.canceled_at";

fn map_visit_row(r: (i64, i64, i64, i64, String, String, i64, f64, f64, String, Option<String>, i64, i8, String, Option<String>, Option<String>, String, Option<String>, Option<String>, Option<String>, Option<String>)) -> VisitRow {
    VisitRow {
        id: r.0, user_id: r.1, ustadz_id: r.2, service_type_id: r.3, service_name: r.4,
        scheduled_at: r.5, duration: r.6, lat: r.7, lng: r.8, address_label: r.9, note: r.10,
        price: r.11, anonymized: r.12 != 0, status: r.13, cancel_reason: r.14, decline_reason: r.15,
        created_at: r.16, paid_at: r.17, confirmed_at: r.18, completed_at: r.19, canceled_at: r.20,
    }
}

pub async fn fetch_visit(pool: &MySqlPool, id: i64) -> Result<Option<VisitRow>, AppError> {
    let a: Option<(i64, i64, i64, i64, String, String, i64, f64, f64, String, Option<String>, i64, i8, String)> = sqlx::query_as(
        "SELECT v.id, v.user_id, v.ustadz_id, v.service_type_id, vst.name, DATE_FORMAT(v.scheduled_at, '%Y-%m-%dT%H:%i:%sZ'), v.duration_minutes,          CAST(v.lat AS DOUBLE), CAST(v.lng AS DOUBLE), v.address_label, v.note, v.price_amount, v.anonymized, v.status          FROM ustadz_visits v JOIN visit_service_types vst ON vst.id = v.service_type_id WHERE v.id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(dberr)?;
    let a = match a { Some(a) => a, None => return Ok(None) };
    let b: (Option<String>, Option<String>, String, Option<String>, Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT cancel_reason, decline_reason, DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(paid_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(confirmed_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(completed_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(canceled_at, '%Y-%m-%dT%H:%i:%sZ') FROM ustadz_visits WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(dberr)?;
    Ok(Some(map_visit_row((
        a.0, a.1, a.2, a.3, a.4, a.5, a.6, a.7, a.8, a.9, a.10, a.11, a.12, a.13,
        b.0, b.1, b.2, b.3, b.4, b.5, b.6,
    ))))
}

pub struct PaymentRow {
    pub id: i64,
    pub external_id: String,
    pub invoice_id: Option<String>,
    pub amount: i64,
    pub status: String,
    pub refunded_amount: i64,
    pub subject_id: i64,
}

pub async fn fetch_payment(pool: &MySqlPool, visit_id: i64) -> Result<Option<PaymentRow>, AppError> {
    let row: Option<(i64, String, Option<String>, i64, String, i64, i64)> = sqlx::query_as(
        "SELECT id, external_id, xendit_invoice_id, amount, status, refunded_amount, subject_id \
         FROM payments WHERE subject_type = 'ustadz_visit' AND subject_id = ? ORDER BY id DESC LIMIT 1")
        .bind(visit_id)
        .fetch_optional(pool)
        .await
        .map_err(dberr)?;
    Ok(row.map(|r| PaymentRow { id: r.0, external_id: r.1, invoice_id: r.2, amount: r.3, status: r.4, refunded_amount: r.5, subject_id: r.6 }))
}

async fn fetch_party(pool: &MySqlPool, user_id: i64) -> PartyOut {
    let (name, phone): (String, Option<String>) = sqlx::query_as(
        "SELECT COALESCE(NULLIF(up.full_name, ''), 'Tanpa Nama'), u.phone \
         FROM users u LEFT JOIN user_profiles up ON up.user_id = u.id WHERE u.id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or(("Tanpa Nama".into(), None));
    PartyOut { user_id, full_name: name, phone }
}

#[allow(dead_code)]
pub fn ts(d: chrono::NaiveDateTime) -> String {
    d.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
fn ts_o(d: &Option<chrono::NaiveDateTime>) -> Option<String> {
    d.as_ref().map(|x| ts(*x))
}

/// `contact_open` = status >= CONFIRMED (kontak dua arah terbuka — keputusan #7 Bagian V)
fn contact_open(status: &str) -> bool {
    matches!(status, "CONFIRMED" | "COMPLETED" | "REVIEWED")
}

/// Bangun VisitOut. `viewer` = Some(user_id peserta) / None = admin (lihat semua kecuali koordinat santri utk admin? admin boleh).
async fn visit_out(pool: &MySqlPool, v: &VisitRow, viewer: Option<i64>) -> Result<VisitOut, AppError> {
    let is_admin = matches!(viewer, None);
    let is_ustadz_side = matches!(viewer, Some(uid) if uid == v.ustadz_id);
    let payment = fetch_payment(pool, v.id).await?;
    let mut ustadz = fetch_party(pool, v.ustadz_id).await;
    let mut requester = fetch_party(pool, v.user_id).await;
    // privacy: phone dua arah HANYA setelah CONFIRMED (kecuali admin)
    if !contact_open(&v.status) && !is_admin {
        ustadz.phone = None;
        requester.phone = None;
    }
    let show_coords = is_admin || viewer.is_some();
    Ok(VisitOut {
        id: v.id,
        status: v.status.clone(),
        service_type_id: v.service_type_id,
        service_name: v.service_name.clone(),
        scheduled_at: v.scheduled_at.clone(),
        duration_minutes: v.duration,
        address_label: if v.anonymized { "—".into() } else { v.address_label.clone() },
        note: v.note.clone(),
        price_amount: v.price,
        lat: if show_coords && !v.anonymized { Some(v.lat) } else { None },
        lng: if show_coords && !v.anonymized { Some(v.lng) } else { None },
        anonymized: v.anonymized,
        ustadz: if is_ustadz_side || is_admin || true { Some(ustadz) } else { None },
        requester: Some(requester),
        payment: payment.map(|p| PaymentOut {
            id: p.id,
            external_id: p.external_id,
            status: p.status,
            invoice_url: None, // diisi caller bila perlu (mock/live URL disimpan di payments? tidak — URL tidak disimpan; regenerate via /pay)
            amount: p.amount,
            refunded_amount: p.refunded_amount,
            expires_at: None,
            paid_at: None,
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

/// Publik utk handler: bangun VisitOut utk viewer (Some=peserta / None=admin).
pub async fn visit_detail_out(pool: &MySqlPool, v: &VisitRow, viewer: Option<i64>) -> Result<VisitOut, AppError> {
    visit_out(pool, v, viewer).await
}

// ===================== layanan & nearby =====================

pub async fn list_services(pool: &MySqlPool) -> Result<Vec<ServiceTypeOut>, AppError> {
    let rows: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, code, name, description FROM visit_service_types WHERE active = 1 ORDER BY sort_order, id")
        .fetch_all(pool)
        .await
        .map_err(dberr)?;
    Ok(rows.into_iter().map(|r| ServiceTypeOut { id: r.0, code: r.1, name: r.2, description: r.3 }).collect())
}

pub async fn nearby(
    state: &AppState,
    user_id: i64,
    lat: f64,
    lng: f64,
    service_type_id: Option<i64>,
) -> Result<Vec<NearbyUstadz>, AppError> {
    if !visit_enabled(&state.pool).await? {
        return Err(AppError::Forbidden("modul Pesan Ustadz sedang nonaktif".into()));
    }
    let radius = setting_i64(&state.pool, "visit_radius_km", 5).await.clamp(1, 50) as f64;
    let fresh_h = setting_i64(&state.pool, "visit_location_fresh_hours", 6).await;
    let dlat = radius / 111.0;
    let dlng = radius / (111.0 * lat.to_radians().cos().max(0.2));
    let rows: Vec<(i64, String, f64, f64, i64, String, i64, i64, Option<f64>, i64)> = sqlx::query_as(
        &format!("SELECT u.id, COALESCE(NULLIF(upn.full_name,''),'Ustadz'), CAST(ul.lat AS DOUBLE), CAST(ul.lng AS DOUBLE), \
         uvsr.service_type_id, vst.name, uvsr.price_amount, uvsr.duration_minutes, \
         (SELECT CAST(AVG(vr.rating) AS DOUBLE) FROM visit_reviews vr WHERE vr.reviewee_id = u.id \
            AND vr.direction = 'SANTRI_TO_USTADZ' AND vr.revealed_at IS NOT NULL AND vr.hidden = 0), \
         (SELECT COUNT(*) FROM visit_reviews vr2 WHERE vr2.reviewee_id = u.id \
            AND vr2.direction = 'SANTRI_TO_USTADZ' AND vr2.revealed_at IS NOT NULL AND vr2.hidden = 0) \
         FROM users u \
         JOIN user_profiles upn ON upn.user_id = u.id \
         JOIN ustadz_profiles up ON up.user_id = u.id AND up.verified_at IS NOT NULL \
         JOIN ustadz_visit_settings vs ON vs.ustadz_id = u.id AND vs.is_accepting = 1 \
         JOIN user_locations ul ON ul.user_id = u.id AND ul.recorded_at >= DATE_SUB(UTC_TIMESTAMP(), INTERVAL {fresh_h} HOUR) \
         JOIN ustadz_visit_services uvsr ON uvsr.ustadz_id = u.id AND uvsr.active = 1 \
         JOIN visit_service_types vst ON vst.id = uvsr.service_type_id AND vst.active = 1 \
         WHERE u.status = 'ACTIVE' AND (? IS NULL OR uvsr.service_type_id = ?) \
           AND ul.lat BETWEEN ? AND ? AND ul.lng BETWEEN ? AND ?"))
        .bind(service_type_id).bind(service_type_id)
        .bind(lat - dlat).bind(lat + dlat)
        .bind(lng - dlng).bind(lng + dlng)
        .fetch_all(&state.pool)
        .await
        .map_err(dberr)?;

    let _ = user_id;
    use std::collections::BTreeMap;
    let mut map: BTreeMap<i64, NearbyUstadz> = BTreeMap::new();
    for r in rows {
        let dist = haversine_km(lat, lng, r.2, r.3);
        if dist > radius {
            continue;
        }
        let e = map.entry(r.0).or_insert_with(|| NearbyUstadz {
            ustadz_id: r.0,
            full_name: r.1.clone(),
            distance_km: dist,
            rating_avg: r.8,
            rating_count: r.9,
            services: vec![],
        });
        e.distance_km = e.distance_km.min(dist);
        e.services.push(TarifOut {
            service_type_id: r.4,
            service_type_name: r.5,
            price_amount: r.6,
            duration_minutes: r.7,
        });
    }
    let mut out: Vec<NearbyUstadz> = map.into_values().collect();
    out.sort_by(|a, b| a.distance_km.partial_cmp(&b.distance_km).unwrap_or(std::cmp::Ordering::Equal));
    Ok(out)
}

// ===================== create / list / detail =====================

pub struct CreateOutcome {
    pub out: VisitCreatedOut,
}

pub async fn create_visit(
    state: &AppState,
    user_id: i64,
    req: CreateVisitReq,
    idem_key: &str,
) -> Result<CreateOutcome, AppError> {
    if !visit_enabled(&state.pool).await? {
        return Err(AppError::Forbidden("modul Pesan Ustadz sedang nonaktif".into()));
    }
    if !state.visit_create_gate(&user_id) {
        return Err(AppError::RateLimited);
    }
    // idempotency: pre-SELECT dulu (gotcha sqlx: ODKU affected_rows tak andal)
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM ustadz_visits WHERE user_id = ? AND client_key = ?")
        .bind(user_id).bind(idem_key)
        .fetch_optional(&state.pool).await.map_err(dberr)?;
    if let Some(vid) = existing {
        let v = fetch_visit(&state.pool, vid).await?.ok_or_else(|| AppError::NotFound("booking hilang".into()))?;
        let p = fetch_payment(&state.pool, vid).await?;
        let out = VisitCreatedOut {
            visit: visit_out(&state.pool, &v, Some(user_id)).await?,
            invoice_url: p.as_ref().and_then(|_| None), // invoice URL tidak disimpan — regenerate /pay
            replay: true,
        };
        return Ok(CreateOutcome { out });
    }
    // validasi layanan
    let svc: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT uvsr.price_amount, uvsr.duration_minutes, vst.active FROM ustadz_visit_services uvsr \
         JOIN visit_service_types vst ON vst.id = uvsr.service_type_id \
         WHERE uvsr.ustadz_id = ? AND uvsr.service_type_id = ? AND uvsr.active = 1")
        .bind(req.ustadz_id).bind(req.service_type_id)
        .fetch_optional(&state.pool).await.map_err(dberr)?;
    let (price, duration) = match svc {
        Some((p, d, 1)) if p >= MIN_TARIF => (p, d),
        Some((p, _, 1)) => return Err(AppError::Unprocessable(format!("tarif layanan belum memenuhi minimal Rp {MIN_TARIF} (saat ini Rp {p})"))),
        _ => return Err(AppError::Unprocessable("ustadz belum membuka layanan ini".into())),
    };
    // jadwal window
    let sched = parse_iso(&req.scheduled_at)?;
    let min_h = setting_i64(&state.pool, "visit_min_schedule_hours", 2).await;
    let max_d = setting_i64(&state.pool, "visit_max_schedule_days", 14).await;
    let n: (i64, i64,) = sqlx::query_as(
        "SELECT TIMESTAMPDIFF(MINUTE, UTC_TIMESTAMP(), ?), TIMESTAMPDIFF(HOUR, UTC_TIMESTAMP(), ?)")
        .bind(sched).bind(sched)
        .fetch_one(&state.pool).await.map_err(dberr)?;
    if n.0 < min_h * 60 {
        return Err(AppError::Unprocessable(format!("jadwal minimal {min_h} jam dari sekarang")));
    }
    if n.1 > max_d * 24 {
        return Err(AppError::Unprocessable(format!("jadwal maksimal {max_d} hari ke depan")));
    }
    // ustadz valid + lokasi fresh + jarak dalam radius
    let radius = setting_i64(&state.pool, "visit_radius_km", 5).await.clamp(1, 50) as f64;
    let fresh_h = setting_i64(&state.pool, "visit_location_fresh_hours", 6).await;
    let uloc: Option<(f64, f64)> = sqlx::query_as(
        &format!("SELECT CAST(ul.lat AS DOUBLE), CAST(ul.lng AS DOUBLE) \
         FROM users u \
         JOIN user_profiles upn ON upn.user_id = u.id \
         JOIN ustadz_profiles up ON up.user_id = u.id AND up.verified_at IS NOT NULL \
         JOIN ustadz_visit_settings vs ON vs.ustadz_id = u.id AND vs.is_accepting = 1 \
         JOIN user_locations ul ON ul.user_id = u.id AND ul.recorded_at >= DATE_SUB(UTC_TIMESTAMP(), INTERVAL {fresh_h} HOUR) \
         WHERE u.id = ? AND u.status = 'ACTIVE'"))
        .bind(req.ustadz_id)
        .fetch_optional(&state.pool).await.map_err(dberr)?;
    let (ulat, ulng) = uloc.ok_or_else(|| AppError::Unprocessable("ustadz sedang tidak dapat dipesan (offline / lokasi tidak segar)".into()))?;
    let dist = haversine_km(req.lat, req.lng, ulat, ulng);
    if dist > radius {
        return Err(AppError::Unprocessable(format!("lokasi Anda di luar radius layanan ustadz ({:.1} km > {radius} km)", dist.ceil())));
    }
    // INSERT visit (DB-enforced max-1-aktif + idempotency)
    let ins = sqlx::query(
        "INSERT INTO ustadz_visits (user_id, ustadz_id, service_type_id, scheduled_at, duration_minutes, \
         lat, lng, address_label, note, client_key, price_amount, status) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'REQUESTED')")
        .bind(user_id).bind(req.ustadz_id).bind(req.service_type_id).bind(sched)
        .bind(duration).bind(req.lat).bind(req.lng)
        .bind(req.address_label.trim()).bind(req.note.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .bind(idem_key).bind(price)
        .execute(&state.pool).await;
    let visit_id = match ins {
        Ok(r) => r.last_insert_id() as i64,
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("uv_active_uq") {
                return Err(AppError::Conflict("masih ada pesanan aktif — selesaikan/batalkan dulu".into()));
            }
            if msg.contains("uv_client_uq") {
                // race replay: ambil existing
                let vid: (i64,) = sqlx::query_as("SELECT id FROM ustadz_visits WHERE user_id = ? AND client_key = ?")
                    .bind(user_id).bind(idem_key)
                    .fetch_one(&state.pool).await.map_err(dberr)?;
                let v = fetch_visit(&state.pool, vid.0).await?.ok_or_else(|| AppError::NotFound("booking hilang".into()))?;
                return Ok(CreateOutcome {
                    out: VisitCreatedOut { visit: visit_out(&state.pool, &v, Some(user_id)).await?, invoice_url: None, replay: true },
                });
            }
            return Err(dberr(e));
        }
    };
    log_history(&state.pool, visit_id, None, "REQUESTED", Some(user_id), "booking dibuat").await?;
    // payment + invoice (via modul payments)
    let created = crate::modules::visits::payments::create_payment_for_visit(state, visit_id, price).await?;
    let v = fetch_visit(&state.pool, visit_id).await?.unwrap();
    Ok(CreateOutcome {
        out: VisitCreatedOut {
            visit: visit_out(&state.pool, &v, Some(user_id)).await?,
            invoice_url: created,
            replay: false,
        },
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
        if (i as i64) == limit {
            next = Some(id.to_string());
            break;
        }
        if let Some(v) = fetch_visit(pool, id).await? {
            out.push(visit_out(pool, &v, Some(user_id)).await?);
        }
    }
    Ok((out, next))
}

/// ACL: peserta (santri/ustadz) atau admin.
pub async fn require_visit_access(pool: &MySqlPool, user: &crate::middleware::auth::CurrentUser, visit_id: i64) -> Result<VisitRow, AppError> {
    let v = fetch_visit(pool, visit_id).await?.ok_or_else(|| AppError::NotFound("pesanan tidak ada".into()))?;
    let is_party = v.user_id == user.user_id || v.ustadz_id == user.user_id;
    if !is_party && !user.permissions.contains("visits.admin") {
        return Err(AppError::NotFound("pesanan tidak ada".into())); // 404 anti-guess
    }
    Ok(v)
}

// ===================== cancel =====================

pub async fn cancel(state: &AppState, user: &crate::middleware::auth::CurrentUser, visit_id: i64) -> Result<VisitOut, AppError> {
    let v = require_visit_access(&state.pool, user, visit_id).await?;
    let now_utc: chrono::NaiveDateTime = sqlx::query_scalar("SELECT UTC_TIMESTAMP()")
        .fetch_one(&state.pool).await.map_err(dberr)?;
    match v.status.as_str() {
        "REQUESTED" => {
            // belum dibayar: void payment aktif tanpa refund
            let mut tx = state.pool.begin().await.map_err(dberr)?;
            sqlx::query("UPDATE payments SET status = 'EXPIRED' WHERE subject_type = 'ustadz_visit' AND subject_id = ? AND status = 'PENDING'")
                .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?;
            sqlx::query("UPDATE ustadz_visits SET status = 'CANCELED', canceled_at = UTC_TIMESTAMP(), canceled_by = ?, cancel_reason = 'dibatalkan sebelum pembayaran'")
                .bind(user.user_id).execute(&mut *tx).await.map_err(dberr)?;
            log_history(&mut *tx, visit_id, Some("REQUESTED"), "CANCELED", Some(user.user_id), "cancel sebelum bayar").await?;
            tx.commit().await.map_err(dberr)?;
        }
        "WAITING_CONFIRM" | "CONFIRMED" => {
            let free_h = setting_i64(&state.pool, "visit_cancel_free_hours", 2).await;
            let mins_left: i64 = sqlx::query_scalar("SELECT TIMESTAMPDIFF(MINUTE, UTC_TIMESTAMP(), ?)")
                .bind(&v.scheduled_at).fetch_one(&state.pool).await.map_err(dberr)?;
            let refund_full = v.status == "WAITING_CONFIRM" || mins_left >= free_h * 60;
            let mut tx = state.pool.begin().await.map_err(dberr)?;
            sqlx::query("UPDATE ustadz_visits SET status = 'CANCELED', canceled_at = UTC_TIMESTAMP(), canceled_by = ?, cancel_reason = ?")
                .bind(user.user_id)
                .bind(if refund_full { "dibatalkan — dana dikembalikan penuh" } else { "dibatalkan kurang dari batas gratis — dana ke ustadz" })
                .execute(&mut *tx).await.map_err(dberr)?;
            log_history(&mut *tx, visit_id, Some(v.status.as_str()), "CANCELED", Some(user.user_id), if refund_full { "refund penuh" } else { "tanpa refund (< batas gratis)" }).await?;
            tx.commit().await.map_err(dberr)?;
            if refund_full {
                crate::modules::visits::payments::attempt_refund(state, visit_id, "santri cancel").await?;
            }
            let v2 = fetch_visit(&state.pool, visit_id).await?.unwrap();
            let (uid,) : (i64,) = (v2.ustadz_id,);
            notify(&state.pool, uid, "VISIT_CANCELED", "Kunjungan dibatalkan",
                &format!("Kunjungan {} dibatalkan oleh santri. {}", v.scheduled_at,
                    if refund_full { "Dana dikembalikan ke santri." } else { "Dana kunjungan menjadi milik Anda." }),
                &visit_id.to_string()).await?;
        }
        other => return Err(AppError::Unprocessable(format!("status {other} tidak bisa dibatalkan"))),
    }
    let _ = now_utc;
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
        "SELECT id, sender_id, body, DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(read_at, '%Y-%m-%dT%H:%i:%sZ') FROM visit_messages \
         WHERE visit_id = ? AND (? IS NULL OR id < ?) ORDER BY id DESC LIMIT ?")
        .bind(visit_id).bind(cursor).bind(cursor).bind(limit + 1)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    let mut next = None;
    for (i, r) in rows.into_iter().enumerate() {
        if (i as i64) == limit {
            next = Some(r.0.to_string());
            break;
        }
        out.push(MessageOut { id: r.0, sender_id: r.1, body: r.2, created_at: r.3, read_at: r.4 });
    }
    out.reverse();
    // tandai read utk pesan lawan (badge unread)
    if v.status != "CANCELED" {
        sqlx::query("UPDATE visit_messages SET read_at = UTC_TIMESTAMP() WHERE visit_id = ? AND sender_id != ? AND read_at IS NULL")
            .bind(visit_id).bind(user.user_id)
            .execute(pool).await.map_err(dberr)?;
    }
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

// ===================== review (dua arah double-blind) =====================

/// Reviewer bisa santri (direction SANTRI_TO_USTADZ) atau ustadz (USTADZ_TO_SANTRI) — otomatis dari relasi visit.
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
    // reveal bila kedua arah sudah submit
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
        .bind(visit_id).bind(user_id)
        .fetch_optional(pool).await.map_err(dberr)?;
    let counterpart: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM visit_reviews WHERE visit_id = ? AND reviewer_id != ?")
        .bind(visit_id).bind(user_id)
        .fetch_one(pool).await.map_err(dberr)?;
    Ok(ReviewStatusOut {
        can_review: v.status == "COMPLETED" && mine.is_none(),
        my_rating: mine.as_ref().map(|m| m.0),
        my_comment: mine.as_ref().and_then(|m| m.1.clone()),
        counterpart_submitted: counterpart > 0,
        revealed: mine.as_ref().map(|m| m.2 != 0).unwrap_or(v.status == "REVIEWED"),
    })
}

/// Publik ke santri: review SANTRI→USTADZ (revealed & !hidden) — layar pilih ustadz.
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
        if (i as i64) == limit {
            next = Some(r.0.to_string());
            break;
        }
        out.push(ReviewPublicOut {
            id: r.0,
            reviewer_first_name: r.1.unwrap_or_else(|| "Santri".into()).split(' ').next().unwrap_or("Santri").to_string(),
            rating: r.2,
            comment: r.3,
            created_at: r.4,
        });
    }
    Ok((out, next))
}

/// Khusus ustadz pemegang request: review USTADZ→SANTRI tentang santri pemesan.
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
        if (i as i64) == limit {
            next = Some(r.0.to_string());
            break;
        }
        out.push(ReviewPublicOut {
            id: r.0,
            reviewer_first_name: r.1.unwrap_or_else(|| "Ustadz".into()).split(' ').next().unwrap_or("Ustadz").to_string(),
            rating: r.2,
            comment: r.3,
            created_at: r.4,
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

// ===================== ustadz side =====================

pub async fn get_visit_settings(pool: &MySqlPool, ustadz_id: i64) -> Result<VisitSettingsOut, AppError> {
    let row: Option<(i8, i64)> = sqlx::query_as(
        "SELECT is_accepting, max_active_visits FROM ustadz_visit_settings WHERE ustadz_id = ?")
        .bind(ustadz_id).fetch_optional(pool).await.map_err(dberr)?;
    Ok(match row {
        Some((a, m)) => VisitSettingsOut { is_accepting: a != 0, max_active_visits: m },
        None => VisitSettingsOut { is_accepting: false, max_active_visits: 2 },
    })
}

pub async fn put_visit_settings(pool: &MySqlPool, ustadz_id: i64, req: VisitSettingsReq) -> Result<VisitSettingsOut, AppError> {
    if !(1..=10).contains(&req.max_active_visits) {
        return Err(AppError::Unprocessable("max_active_visits 1-10".into()));
    }
    sqlx::query(
        "INSERT INTO ustadz_visit_settings (ustadz_id, is_accepting, max_active_visits) VALUES (?, ?, ?) AS new \
         ON DUPLICATE KEY UPDATE is_accepting = new.is_accepting, max_active_visits = new.max_active_visits")
        .bind(ustadz_id).bind(req.is_accepting as i8).bind(req.max_active_visits)
        .execute(pool).await.map_err(dberr)?;
    Ok(VisitSettingsOut { is_accepting: req.is_accepting, max_active_visits: req.max_active_visits })
}

pub async fn list_tarif(pool: &MySqlPool, ustadz_id: i64) -> Result<Vec<TarifOut>, AppError> {
    let rows: Vec<(i64, String, i64, i64)> = sqlx::query_as(
        "SELECT uvsr.service_type_id, vst.name, uvsr.price_amount, uvsr.duration_minutes \
         FROM ustadz_visit_services uvsr JOIN visit_service_types vst ON vst.id = uvsr.service_type_id \
         WHERE uvsr.ustadz_id = ? ORDER BY vst.sort_order")
        .bind(ustadz_id).fetch_all(pool).await.map_err(dberr)?;
    Ok(rows.into_iter().map(|r| TarifOut {
        service_type_id: r.0, service_type_name: r.1, price_amount: r.2, duration_minutes: r.3,
    }).collect())
}

pub async fn upsert_tarif(pool: &MySqlPool, ustadz_id: i64, req: TarifUpsertReq) -> Result<(), AppError> {
    if req.price_amount < MIN_TARIF || req.price_amount > 100_000_000 {
        return Err(AppError::Unprocessable(format!("tarif Rp {MIN_TARIF} - Rp 100.000.000 (tidak ada booking gratis)")));
    }
    if !(15..=600).contains(&req.duration_minutes) {
        return Err(AppError::Unprocessable("durasi 15-600 menit".into()));
    }
    let known: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM visit_service_types WHERE id = ? AND active = 1")
        .bind(req.service_type_id).fetch_one(pool).await.map_err(dberr)?;
    if known == 0 {
        return Err(AppError::Unprocessable("jenis layanan tidak dikenal".into()));
    }
    sqlx::query(
        "INSERT INTO ustadz_visit_services (ustadz_id, service_type_id, price_amount, duration_minutes, note, active) \
         VALUES (?, ?, ?, ?, ?, 1) AS new \
         ON DUPLICATE KEY UPDATE price_amount = new.price_amount, duration_minutes = new.duration_minutes, note = new.note, active = 1")
        .bind(ustadz_id).bind(req.service_type_id).bind(req.price_amount).bind(req.duration_minutes)
        .bind(req.note.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}

pub async fn delete_tarif(pool: &MySqlPool, ustadz_id: i64, service_type_id: i64) -> Result<(), AppError> {
    let n = sqlx::query("DELETE FROM ustadz_visit_services WHERE ustadz_id = ? AND service_type_id = ?")
        .bind(ustadz_id).bind(service_type_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Err(AppError::NotFound("tarif tidak ada".into()));
    }
    Ok(())
}

fn requester_agg(sub_direction: &'static str) -> String {
    format!(
        "(SELECT CAST(AVG(vr.rating) AS DOUBLE) FROM visit_reviews vr WHERE vr.reviewee_id = {{uid}} AND vr.direction = '{sub_direction}' AND vr.revealed_at IS NOT NULL AND vr.hidden = 0), \
         (SELECT COUNT(*) FROM visit_reviews vr2 WHERE vr2.reviewee_id = {{uid}} AND vr2.direction = '{sub_direction}' AND vr2.revealed_at IS NOT NULL AND vr2.hidden = 0)")
}

pub async fn my_visits(pool: &MySqlPool, ustadz_id: i64) -> Result<MyVisitsOut, AppError> {
    // incoming: WAITING_CONFIRM + requester profile + rating agregat santri
    let agg = requester_agg("USTADZ_TO_SANTRI").replace("{uid}", "v.user_id");
    let sql_in = format!(
        "SELECT v.id, v.status, vst.name, DATE_FORMAT(v.scheduled_at, '%Y-%m-%dT%H:%i:%sZ'), v.duration_minutes, v.price_amount, v.note, \
         v.user_id, COALESCE(NULLIF(up.full_name,''),'Santri'), {agg} \
         FROM ustadz_visits v \
         JOIN visit_service_types vst ON vst.id = v.service_type_id \
         JOIN users u ON u.id = v.user_id \
         LEFT JOIN user_profiles up ON up.user_id = v.user_id \
         WHERE v.ustadz_id = ? AND v.status = 'WAITING_CONFIRM' ORDER BY v.scheduled_at ASC LIMIT 50");
    let rows: Vec<(i64, String, String, String, i64, i64, Option<String>, i64, String, Option<f64>, i64)> =
        sqlx::query_as(&sql_in).bind(ustadz_id).fetch_all(pool).await.map_err(dberr)?;
    let incoming = rows.into_iter().map(|r| IncomingVisitOut {
        id: r.0, status: r.1, service_name: r.2, scheduled_at: r.3, duration_minutes: r.4,
        price_amount: r.5, note: r.6,
        requester: RequesterOut { user_id: r.7, full_name: r.8, rating_avg: r.9, rating_count: r.10 },
    }).collect();
    // upcoming: CONFIRMED
    let rows2: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM ustadz_visits WHERE ustadz_id = ? AND status = 'CONFIRMED' ORDER BY scheduled_at ASC LIMIT 50")
        .bind(ustadz_id).fetch_all(pool).await.map_err(dberr)?;
    let mut upcoming = Vec::new();
    for (id,) in rows2 {
        if let Some(v) = fetch_visit(pool, id).await? {
            upcoming.push(visit_out(pool, &v, Some(ustadz_id)).await?);
        }
    }
    Ok(MyVisitsOut { incoming, upcoming })
}

pub async fn confirm_visit(state: &AppState, ustadz_user_id: i64, visit_id: i64) -> Result<VisitOut, AppError> {
    let mut tx = state.pool.begin().await.map_err(dberr)?;
    // lock serialisasi per ustadz (plan rev 5): FOR UPDATE pada row settings
    let settings: Option<(i8, i64)> = sqlx::query_as(
        "SELECT is_accepting, max_active_visits FROM ustadz_visit_settings WHERE ustadz_id = ? FOR UPDATE")
        .bind(ustadz_user_id)
        .fetch_optional(&mut *tx).await.map_err(dberr)?;
    let (_, max_active) = settings.ok_or_else(|| AppError::Forbidden("aktifkan dulu pengaturan kunjungan (accepting)".into()))?;
    let v = fetch_visit_tx(&mut tx, visit_id).await?.ok_or_else(|| AppError::NotFound("pesanan tidak ada".into()))?;
    if v.ustadz_id != ustadz_user_id {
        return Err(AppError::NotFound("pesanan tidak ada".into()));
    }
    if v.status != "WAITING_CONFIRM" {
        return Err(AppError::Conflict(format!("status {}, harus WAITING_CONFIRM", v.status)));
    }
    // kapasitas aktif
    let active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ustadz_visits WHERE ustadz_id = ? AND status = 'CONFIRMED'")
        .bind(ustadz_user_id).fetch_one(&mut *tx).await.map_err(dberr)?;
    if active >= max_active {
        return Err(AppError::Conflict("ustadz_active_limit: kapasitas kunjungan aktif penuh".into()));
    }
    // overlap: interval CONFIRMED beririsan dgn [sched, sched+dur)
    let overlap: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ustadz_visits WHERE ustadz_id = ? AND status = 'CONFIRMED' AND id != ? \
         AND scheduled_at < DATE_ADD(?, INTERVAL ? MINUTE) \
         AND DATE_ADD(scheduled_at, INTERVAL duration_minutes MINUTE) > ?")
        .bind(ustadz_user_id).bind(visit_id).bind(&v.scheduled_at).bind(v.duration).bind(&v.scheduled_at)
        .fetch_one(&mut *tx).await.map_err(dberr)?;
    if overlap > 0 {
        return Err(AppError::Conflict("ustadz_schedule_conflict: bentrok jadwal dengan kunjungan lain".into()));
    }
    let n = sqlx::query("UPDATE ustadz_visits SET status = 'CONFIRMED', confirmed_at = UTC_TIMESTAMP() WHERE id = ? AND status = 'WAITING_CONFIRM'")
        .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Err(AppError::Conflict("pesanan sudah tidak menunggu konfirmasi".into()));
    }
    log_history(&mut *tx, visit_id, Some("WAITING_CONFIRM"), "CONFIRMED", Some(ustadz_user_id), "ustadz mengonfirmasi").await?;
    notify(&mut *tx, v.user_id, "VISIT_CONFIRMED", "Kunjungan dikonfirmasi",
        &format!("Ustadz telah MENGONFIRMASI kunjungan {}. Kontak & chat kini terbuka.", v.scheduled_at), &visit_id.to_string()).await?;
    tx.commit().await.map_err(dberr)?;
    let v2 = fetch_visit(&state.pool, visit_id).await?.unwrap();
    Ok(visit_out(&state.pool, &v2, Some(ustadz_user_id)).await?)
}

async fn fetch_visit_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    id: i64,
) -> Result<Option<VisitRow>, AppError> {
    let a: Option<(i64, i64, i64, i64, String, String, i64, f64, f64, String, Option<String>, i64, i8, String)> = sqlx::query_as(
        "SELECT v.id, v.user_id, v.ustadz_id, v.service_type_id, vst.name, DATE_FORMAT(v.scheduled_at, '%Y-%m-%dT%H:%i:%sZ'), v.duration_minutes,          CAST(v.lat AS DOUBLE), CAST(v.lng AS DOUBLE), v.address_label, v.note, v.price_amount, v.anonymized, v.status          FROM ustadz_visits v JOIN visit_service_types vst ON vst.id = v.service_type_id WHERE v.id = ? FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(dberr)?;
    let a = match a { Some(a) => a, None => return Ok(None) };
    let b: (Option<String>, Option<String>, String, Option<String>, Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT cancel_reason, decline_reason, DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(paid_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(confirmed_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(completed_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(canceled_at, '%Y-%m-%dT%H:%i:%sZ') FROM ustadz_visits WHERE id = ? FOR UPDATE")
        .bind(id)
        .fetch_one(&mut **tx)
        .await
        .map_err(dberr)?;
    Ok(Some(map_visit_row((
        a.0, a.1, a.2, a.3, a.4, a.5, a.6, a.7, a.8, a.9, a.10, a.11, a.12, a.13,
        b.0, b.1, b.2, b.3, b.4, b.5, b.6,
    ))))
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
    log_history(&mut *tx, visit_id, Some("WAITING_CONFIRM"), "DECLINED", Some(ustadz_user_id), "ustadz menolak — refund penuh").await?;
    tx.commit().await.map_err(dberr)?;
    crate::modules::visits::payments::attempt_refund(state, visit_id, "ustadz menolak").await?;
    notify(&state.pool, v.user_id, "VISIT_DECLINED_REFUNDED", "Permintaan ditolak ustadz",
        &format!("Ustadz menolak kunjungan {}. Dana PENUH dikembalikan ke metode pembayaran Anda.", v.scheduled_at), &visit_id.to_string()).await?;
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
    let in_window: i64 = sqlx::query_scalar(
        "SELECT CASE WHEN UTC_TIMESTAMP() >= DATE_ADD(?, INTERVAL -30 MINUTE) \
              AND UTC_TIMESTAMP() <= DATE_ADD(DATE_ADD(?, INTERVAL duration_minutes MINUTE), INTERVAL 24 HOUR) \
         THEN 1 ELSE 0 END FROM ustadz_visits WHERE id = ?")
        .bind(&v.scheduled_at).bind(&v.scheduled_at).bind(visit_id)
        .fetch_one(&state.pool).await.map_err(dberr)?;
    if in_window == 0 {
        return Err(AppError::Unprocessable("di luar jadwal — hubungi admin (force-complete)".into()));
    }
    let mut tx = state.pool.begin().await.map_err(dberr)?;
    sqlx::query("UPDATE ustadz_visits SET status = 'COMPLETED', completed_at = UTC_TIMESTAMP() WHERE id = ? AND status = 'CONFIRMED'")
        .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?;
    log_history(&mut *tx, visit_id, Some("CONFIRMED"), "COMPLETED", Some(ustadz_user_id), "kunjungan selesai").await?;
    notify(&mut *tx, v.user_id, "VISIT_COMPLETED_PLEASE_REVIEW", "Kunjungan selesai",
        &format!("Alhamdulillah, kunjungan {} selesai. Beri rating & catatan untuk ustadz Anda.", v.scheduled_at), &visit_id.to_string()).await?;
    notify(&mut *tx, ustadz_user_id, "VISIT_REVIEW_USTADZ_PENDING", "Nilai santri Anda",
        &format!("Kunjungan {} sudah selesai. Beri rating & catatan untuk santri Anda (privat, double-blind).", v.scheduled_at), &visit_id.to_string()).await?;
    tx.commit().await.map_err(dberr)?;
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
        if (i as i64) == limit {
            next = Some(id.to_string());
            break;
        }
        if let Some(v) = fetch_visit(pool, id).await? {
            out.push(visit_out(pool, &v, None).await?);
        }
    }
    Ok((out, next))
}

pub async fn admin_force_complete(pool: &MySqlPool, admin_id: i64, visit_id: i64) -> Result<VisitOut, AppError> {
    let v = fetch_visit(pool, visit_id).await?.ok_or_else(|| AppError::NotFound("pesanan tidak ada".into()))?;
    if !matches!(v.status.as_str(), "CONFIRMED" | "COMPLETED" | "REVIEWED") {
        return Err(AppError::Unprocessable(format!("status {} tidak bisa dipaksa selesai", v.status)));
    }
    if v.status == "CONFIRMED" {
        let mut tx = pool.begin().await.map_err(dberr)?;
        sqlx::query("UPDATE ustadz_visits SET status = 'COMPLETED', completed_at = UTC_TIMESTAMP() WHERE id = ? AND status = 'CONFIRMED'")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?;
        log_history(&mut *tx, visit_id, Some("CONFIRMED"), "COMPLETED", Some(admin_id), "ADMIN force-complete").await?;
        notify(&mut *tx, v.user_id, "VISIT_COMPLETED_PLEASE_REVIEW", "Kunjungan selesai (admin)",
            &format!("Kunjungan {} ditandai selesai oleh admin. Beri rating untuk ustadz Anda.", v.scheduled_at), &visit_id.to_string()).await?;
        tx.commit().await.map_err(dberr)?;
    }
    let v2 = fetch_visit(pool, visit_id).await?.unwrap();
    Ok(visit_out(pool, &v2, None).await?)
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
        crate::modules::visits::payments::attempt_refund(state, visit_id, "admin force-cancel").await?;
    }
    let v2 = fetch_visit(&state.pool, visit_id).await?.unwrap();
    Ok(visit_out(&state.pool, &v2, None).await?)
}

pub async fn admin_list_reviews(pool: &MySqlPool, direction: Option<&str>, limit: i64, cursor: Option<i64>) -> Result<(Vec<serde_json::Value>, Option<String>), AppError> {
    let rows: Vec<(i64, String, i64, i64, i8, Option<String>, i8, i8, String)> = sqlx::query_as(
        "SELECT vr.id, vr.direction, vr.reviewer_id, vr.reviewee_id, vr.rating, vr.comment, vr.hidden, vr.revealed_at IS NOT NULL, DATE_FORMAT(vr.created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM visit_reviews vr WHERE (? IS NULL OR vr.direction = ?) AND (? IS NULL OR vr.id < ?) ORDER BY vr.id DESC LIMIT ?")
        .bind(direction).bind(direction).bind(cursor).bind(cursor).bind(limit + 1)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    let mut next = None;
    for (i, r) in rows.into_iter().enumerate() {
        if (i as i64) == limit {
            next = Some(r.0.to_string());
            break;
        }
        out.push(serde_json::json!({
            "id": r.0, "direction": r.1, "reviewer_id": r.2, "reviewee_id": r.3,
            "rating": r.4, "comment": r.5, "hidden": r.6 != 0,
            "revealed": r.7 != 0, "created_at": r.8,
        }));
    }
    Ok((out, next))
}

pub async fn admin_set_review_hidden(pool: &MySqlPool, review_id: i64, hidden: bool) -> Result<(), AppError> {
    let n = sqlx::query("UPDATE visit_reviews SET hidden = ? WHERE id = ?")
        .bind(hidden as i8).bind(review_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Err(AppError::NotFound("review tidak ada".into()));
    }
    Ok(())
}
