//! Service modul khatmil — rev 3.3: claim juz = INSERT murni (unique marker-aktif);
//! completion v2 POSISI-ONLY (KEPUTUSAN USER OPSI B): COMPLETED <=> posisi == ayat terakhir juz;
//! legacy pages+minutes rule DIMATIKAN (pages/minutes = telemetri saja).
use sqlx::MySqlPool;

use crate::modules::khatmil::dto::*;
use crate::shared::error::AppError;

fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}")
    )
}

// =====================================================================
// Calc engine posisi (rev 3.x) — master: quran_juzs + quran_ayahs
// =====================================================================

async fn juz_bounds(pool: &MySqlPool, juz: i64) -> Result<(i64, i64, i64, i64), AppError> {
    let row: Option<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT start_surah_id, start_ayah, end_surah_id, end_ayah FROM quran_juzs WHERE id = ?")
        .bind(juz).fetch_optional(pool).await.map_err(dberr)?;
    row.ok_or_else(|| AppError::Internal("master quran_juzs tidak ditemukan".into()))
}

async fn juz_total_ayat(pool: &MySqlPool, juz: i64) -> Result<i64, AppError> {
    sqlx::query_scalar("SELECT COUNT(*) FROM quran_ayahs WHERE juz = ?")
        .bind(juz).fetch_one(pool).await.map_err(dberr)
}

async fn ayat_offset(pool: &MySqlPool, juz: i64, surah: i64, ayah: i64) -> Result<i64, AppError> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM quran_ayahs WHERE juz = ? AND (surah_id * 1000 + ayah_number) <= ?")
        .bind(juz).bind(surah * 1000 + ayah)
        .fetch_one(pool).await.map_err(dberr)
}

/// Validasi posisi (surah, ayah) berada dalam rentang juz — 422 + pesan rentang bila tidak.
async fn ensure_position_in_juz(pool: &MySqlPool, juz: i64, surah: i64, ayah: i64) -> Result<(), AppError> {
    let (ss, sa, es, ea) = juz_bounds(pool, juz).await?;
    if (surah, ayah) < (ss, sa) || (surah, ayah) > (es, ea) {
        return Err(AppError::Unprocessable(format!(
            "posisi harus dalam Juz {juz}: QS {ss}:{sa} s.d. QS {es}:{ea}"
        )));
    }
    Ok(())
}

fn pct(part: i64, total: i64) -> Option<f64> {
    if total <= 0 { return None; }
    Some((part as f64 * 100.0 / total as f64 * 10.0).round() / 10.0)
}

// =====================================================================
// Campaign list / create / update (lifecycle guard rev 3.2)
// =====================================================================

pub async fn list_campaigns(pool: &MySqlPool, status: Option<String>) -> Result<Vec<CampaignOut>, AppError> {
    let rows: Vec<(i64, String, String, String, String, i64, i64, i8, i64, i64, f64)> = sqlx::query_as(
        "SELECT c.id, c.slug, c.name, c.mode, c.status, c.target_khataman, c.min_minutes_per_juz, c.require_manual_verification, \
         (SELECT COUNT(*) FROM khatmil_participants p WHERE p.campaign_id = c.id), \
         (SELECT COUNT(*) FROM khatmil_juz_assignments a WHERE a.campaign_id = c.id AND a.status = 'COMPLETED'), \
         CAST(IFNULL(ROUND(100.0 * (SELECT COUNT(*) FROM khatmil_juz_assignments a2 WHERE a2.campaign_id = c.id AND a2.status = 'COMPLETED') / 30, 1), 0) AS DOUBLE) \
         FROM khatmil_campaigns c WHERE (? IS NULL OR c.status = ?) ORDER BY c.id DESC")
        .bind(status.as_deref()).bind(status.as_deref())
        .fetch_all(pool).await.map_err(dberr)?;
    Ok(rows.into_iter().map(|r| CampaignOut {
        id: r.0, slug: r.1, name: r.2, mode: r.3, status: r.4, target_khataman: r.5,
        min_minutes_per_juz: r.6, require_manual_verification: r.7 != 0,
        participants: r.8, juz_completed: r.9, progress_pct: r.10,
    }).collect())
}

pub async fn create_campaign(pool: &MySqlPool, created_by: i64, req: CampaignUpsertReq) -> Result<i64, AppError> {
    validate_upsert(&req)?;
    let dup: Option<(i64,)> = sqlx::query_as("SELECT id FROM khatmil_campaigns WHERE slug = ?")
        .bind(&req.slug).fetch_optional(pool).await.map_err(dberr)?;
    if dup.is_some() {
        return Err(AppError::Conflict("slug sudah dipakai".into()));
    }
    let ins = sqlx::query(
        "INSERT INTO khatmil_campaigns (slug, name, description, mode, status, target_khataman, \
         period_start, period_end, min_minutes_per_juz, require_manual_verification, max_participants, created_by) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&req.slug).bind(&req.name).bind(&req.description)
        .bind(&req.mode).bind(&req.status).bind(req.target_khataman)
        .bind(parse_date(req.period_start.as_deref())?).bind(parse_date(req.period_end.as_deref())?)
        .bind(req.min_minutes_per_juz).bind(req.require_manual_verification)
        .bind(req.max_participants).bind(created_by)
        .execute(pool).await.map_err(dberr)?;
    Ok(ins.last_insert_id() as i64)
}

fn transition_ok(from: &str, to: &str) -> bool {
    from == to || matches!((from, to),
        ("DRAFT", "SCHEDULED") | ("DRAFT", "ACTIVE") | ("SCHEDULED", "ACTIVE")
        | ("SCHEDULED", "CANCELLED") | ("ACTIVE", "COMPLETED") | ("ACTIVE", "CANCELLED"))
}

pub async fn update_campaign(pool: &MySqlPool, id: i64, req: CampaignUpsertReq) -> Result<(), AppError> {
    validate_upsert(&req)?;
    let cur: Option<(String,)> = sqlx::query_as("SELECT status FROM khatmil_campaigns WHERE id = ?")
        .bind(id).fetch_optional(pool).await.map_err(dberr)?;
    let (cur,) = cur.ok_or_else(|| AppError::NotFound("campaign tidak ada".into()))?;
    if !transition_ok(&cur, &req.status) {
        return Err(AppError::Conflict(format!("invalid_status_transition: {cur} -> {}", req.status)));
    }
    let n = sqlx::query(
        "UPDATE khatmil_campaigns SET name = ?, description = ?, mode = ?, status = ?, target_khataman = ?, \
         period_start = ?, period_end = ?, min_minutes_per_juz = ?, require_manual_verification = ?, max_participants = ? WHERE id = ?")
        .bind(&req.name).bind(&req.description).bind(&req.mode).bind(&req.status).bind(req.target_khataman)
        .bind(parse_date(req.period_start.as_deref())?).bind(parse_date(req.period_end.as_deref())?)
        .bind(req.min_minutes_per_juz).bind(req.require_manual_verification).bind(req.max_participants).bind(id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 { return Err(AppError::NotFound("campaign tidak ada".into())); }
    Ok(())
}

fn validate_upsert(req: &CampaignUpsertReq) -> Result<(), AppError> {
    if !matches!(req.mode.as_str(), "PARALLEL" | "SEQUENTIAL") {
        return Err(AppError::Unprocessable("mode PARALLEL/SEQUENTIAL".into()));
    }
    if !matches!(req.status.as_str(), "DRAFT" | "SCHEDULED" | "ACTIVE" | "COMPLETED" | "CANCELLED") {
        return Err(AppError::Unprocessable("status tidak valid".into()));
    }
    if !(1..=600).contains(&req.min_minutes_per_juz) {
        return Err(AppError::Unprocessable("min_minutes_per_juz 1-600".into()));
    }
    Ok(())
}

fn parse_date(d: Option<&str>) -> Result<Option<chrono::NaiveDate>, AppError> {
    match d {
        None => Ok(None),
        Some(s) => chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(Some).map_err(|_| AppError::Unprocessable("tanggal YYYY-MM-DD".into())),
    }
}

// =====================================================================
// Detail campaign — peta juz v2: assignment TERBARU per juz (aktif ATAU COMPLETED terakhir)
// =====================================================================

pub async fn campaign_detail(pool: &MySqlPool, id: i64) -> Result<CampaignDetail, AppError> {
    let base: Option<(i64, String, String, String, String, i64, i64, i8, i64, i64, f64, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT c.id, c.slug, c.name, c.mode, c.status, c.target_khataman, c.min_minutes_per_juz, c.require_manual_verification, \
             (SELECT COUNT(*) FROM khatmil_participants p WHERE p.campaign_id = c.id), \
             (SELECT COUNT(*) FROM khatmil_juz_assignments a WHERE a.campaign_id = c.id AND a.status = 'COMPLETED'), \
             CAST(IFNULL(ROUND(100.0 * (SELECT COUNT(*) FROM khatmil_juz_assignments a2 WHERE a2.campaign_id = c.id AND a2.status = 'COMPLETED') / 30, 1), 0) AS DOUBLE), \
             DATE_FORMAT(c.period_start, '%Y-%m-%d'), DATE_FORMAT(c.period_end, '%Y-%m-%d') \
             FROM khatmil_campaigns c WHERE c.id = ?")
        .bind(id).fetch_optional(pool).await.map_err(dberr)?;
    let (cid, slug, name, mode, status, target, minmin, rmv, participants, completed, pctv, ps, pe) = base
        .ok_or_else(|| AppError::NotFound("campaign tidak ada".into()))?;
    // peta 30 juz — assignment terbaru per juz apa pun statusnya (juz COMPLETED tetap terlihat)
    let rows: Vec<(i64, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<String>, i64, Option<i64>)> =
        sqlx::query_as(
            "SELECT j.j, a.status, \
             (SELECT p2.full_name FROM user_profiles p2 JOIN khatmil_participants pp ON pp.user_id = p2.user_id WHERE pp.id = a.participant_id), \
             pg.pages_read, pg.minutes_read, pg.current_surah_id, pg.current_ayah, DATE_FORMAT(pg.verified_at, '%Y-%m-%dT%H:%i:%sZ'), \
             (SELECT COUNT(*) FROM quran_ayahs qa WHERE qa.juz = j.j), \
             CASE WHEN pg.current_surah_id IS NOT NULL THEN \
               (SELECT COUNT(*) FROM quran_ayahs qb WHERE qb.juz = j.j AND (qb.surah_id * 1000 + qb.ayah_number) <= (pg.current_surah_id * 1000 + pg.current_ayah)) \
             END \
             FROM (SELECT 1 j UNION SELECT 2 UNION SELECT 3 UNION SELECT 4 UNION SELECT 5 UNION SELECT 6 UNION SELECT 7 UNION SELECT 8 UNION SELECT 9 UNION SELECT 10 \
                   UNION SELECT 11 UNION SELECT 12 UNION SELECT 13 UNION SELECT 14 UNION SELECT 15 UNION SELECT 16 UNION SELECT 17 UNION SELECT 18 UNION SELECT 19 UNION SELECT 20 \
                   UNION SELECT 21 UNION SELECT 22 UNION SELECT 23 UNION SELECT 24 UNION SELECT 25 UNION SELECT 26 UNION SELECT 27 UNION SELECT 28 UNION SELECT 29 UNION SELECT 30) j \
             LEFT JOIN khatmil_juz_assignments a ON a.campaign_id = ? AND a.juz = j.j \
                  AND a.id = (SELECT MAX(a2.id) FROM khatmil_juz_assignments a2 WHERE a2.campaign_id = ? AND a2.juz = j.j) \
             LEFT JOIN khatmil_progress pg ON pg.assignment_id = a.id \
             ORDER BY j.j")
        .bind(id).bind(id).fetch_all(pool).await.map_err(dberr)?;
    let juz_map = rows.into_iter().map(|r| JuzSlot {
        juz: r.0, status: r.1, owner_name: r.2, pages_read: r.3, minutes_read: r.4,
        current_surah: r.5, current_ayah: r.6, completed_at: r.7,
        progress_pct: pct(r.9.unwrap_or(0), r.8),
    }).collect();
    Ok(CampaignDetail {
        campaign: CampaignOut {
            id: cid, slug, name, mode, status, target_khataman: target,
            min_minutes_per_juz: minmin, require_manual_verification: rmv != 0,
            participants, juz_completed: completed, progress_pct: pctv,
        },
        juz_map, period_start: ps, period_end: pe,
    })
}

// =====================================================================
// Join + claim (tidak berubah dari v1 — unique marker-aktif jaminan final)
// =====================================================================

pub async fn join(pool: &MySqlPool, user_id: i64, campaign_id: i64) -> Result<(), AppError> {
    let (status, maxp, curp): (String, Option<i64>, i64) = sqlx::query_as(
        "SELECT c.status, c.max_participants, (SELECT COUNT(*) FROM khatmil_participants p WHERE p.campaign_id = c.id) \
         FROM khatmil_campaigns c WHERE c.id = ?")
        .bind(campaign_id).fetch_optional(pool).await.map_err(dberr)?
        .ok_or_else(|| AppError::NotFound("campaign tidak ada".into()))?;
    if status != "ACTIVE" {
        return Err(AppError::Conflict(format!("campaign berstatus {status} — hanya ACTIVE bisa join")));
    }
    if let Some(mx) = maxp {
        if curp >= mx {
            return Err(AppError::Conflict("kuota peserta penuh".into()));
        }
    }
    sqlx::query("INSERT IGNORE INTO khatmil_participants (campaign_id, user_id) VALUES (?, ?)")
        .bind(campaign_id).bind(user_id)
        .execute(pool).await.map_err(dberr)?;
    sqlx::query("INSERT INTO activity_events (user_id, event_type, ref_type, ref_id, payload) \
                 VALUES (?, 'khatmil.joined', 'khatmil_campaign', ?, CAST('{}' AS JSON))")
        .bind(user_id).bind(campaign_id.to_string())
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}

pub async fn claim(pool: &MySqlPool, user_id: i64, campaign_id: i64, juz: Option<i64>) -> Result<AssignmentOut, AppError> {
    let (status, _minmin): (String, i64) = sqlx::query_as(
        "SELECT status, min_minutes_per_juz FROM khatmil_campaigns WHERE id = ?")
        .bind(campaign_id).fetch_optional(pool).await.map_err(dberr)?
        .ok_or_else(|| AppError::NotFound("campaign tidak ada".into()))?;
    if status != "ACTIVE" {
        return Err(AppError::Conflict(format!("campaign berstatus {status}")));
    }
    let participant: (i64,) = sqlx::query_as(
        "SELECT id FROM khatmil_participants WHERE campaign_id = ? AND user_id = ?")
        .bind(campaign_id).bind(user_id)
        .fetch_optional(pool).await.map_err(dberr)?
        .ok_or_else(|| AppError::Unprocessable("join campaign dulu sebelum klaim juz".into()))?;

    let juz = match juz {
        Some(j) => {
            if !(1..=30).contains(&j) {
                return Err(AppError::Unprocessable("juz 1-30".into()));
            }
            j
        }
        None => {
            // auto-assign: juz kosong terkecil (slot aktif = ASSIGNED/IN_PROGRESS)
            let free: Option<(i64,)> = sqlx::query_as(
                "SELECT j.j FROM (SELECT 1 j UNION SELECT 2 UNION SELECT 3 UNION SELECT 4 UNION SELECT 5 UNION SELECT 6 UNION SELECT 7 \
                  UNION SELECT 8 UNION SELECT 9 UNION SELECT 10 UNION SELECT 11 UNION SELECT 12 UNION SELECT 13 UNION SELECT 14 UNION SELECT 15 \
                  UNION SELECT 16 UNION SELECT 17 UNION SELECT 18 UNION SELECT 19 UNION SELECT 20 UNION SELECT 21 UNION SELECT 22 UNION SELECT 23 \
                  UNION SELECT 24 UNION SELECT 25 UNION SELECT 26 UNION SELECT 27 UNION SELECT 28 UNION SELECT 29 UNION SELECT 30) j \
                 WHERE NOT EXISTS (SELECT 1 FROM khatmil_juz_assignments a WHERE a.campaign_id = ? AND a.juz = j.j AND a.status IN ('ASSIGNED','IN_PROGRESS')) \
                 ORDER BY j.j LIMIT 1")
                .bind(campaign_id).fetch_optional(pool).await.map_err(dberr)?;
            free.map(|f| f.0).ok_or_else(|| AppError::Conflict("semua juz sudah diklaim peserta lain".into()))?
        }
    };

    // CLAIM = INSERT murni — partial-unique marker-aktif (generated col) = jaminan final (0009)
    let ins = sqlx::query(
        "INSERT INTO khatmil_juz_assignments (campaign_id, participant_id, juz) VALUES (?, ?, ?)")
        .bind(campaign_id).bind(participant.0).bind(juz)
        .execute(pool).await;
    let ins = match ins {
        Ok(i) => i,
        Err(sqlx::Error::Database(d)) if d.message().contains("khatmil_juz_active_uq") || d.message().contains("Duplicate entry") => {
            return Err(AppError::Conflict(format!("juz {juz} sudah diklaim peserta lain")));
        }
        Err(e) => return Err(dberr(e)),
    };
    // GOTCHA pool: LAST_INSERT_ID() tidak lintas koneksi — pakai result.last_insert_id()
    let aid = ins.last_insert_id() as i64;
    sqlx::query("INSERT IGNORE INTO khatmil_progress (assignment_id) VALUES (?)")
        .bind(aid).execute(pool).await.map_err(dberr)?;
    let total = juz_total_ayat(pool, juz).await?;
    let name: (String,) = sqlx::query_as("SELECT name FROM khatmil_campaigns WHERE id = ?")
        .bind(campaign_id).fetch_one(pool).await.map_err(dberr)?;
    Ok(AssignmentOut {
        id: aid, campaign_id, campaign_name: name.0, juz, status: "ASSIGNED".into(),
        due_at: None, pages_read: 0, minutes_read: 0, verification: "PENDING".into(),
        current_surah: None, current_ayah: None, read_ayat: None, juz_total_ayat: total, progress_pct: None,
    })
}

// =====================================================================
// Assignment out builder (dengan posisi) — dipakai post_progress & my_assignments
// =====================================================================

async fn assignment_out(pool: &MySqlPool, assignment_id: i64) -> Result<AssignmentOut, AppError> {
    let r: (i64, i64, String, i64, String, Option<String>, i64, i64, String, Option<i64>, Option<i64>, i64, Option<i64>) =
        sqlx::query_as(
            "SELECT a.id, a.campaign_id, c.name, a.juz, a.status, DATE_FORMAT(a.due_at, '%Y-%m-%dT%H:%i:%sZ'), \
             IFNULL(pg.pages_read, 0), IFNULL(pg.minutes_read, 0), IFNULL(pg.verification, 'PENDING'), \
             pg.current_surah_id, pg.current_ayah, \
             (SELECT COUNT(*) FROM quran_ayahs qa WHERE qa.juz = a.juz), \
             CASE WHEN pg.current_surah_id IS NOT NULL THEN \
               (SELECT COUNT(*) FROM quran_ayahs qb WHERE qb.juz = a.juz AND (qb.surah_id * 1000 + qb.ayah_number) <= (pg.current_surah_id * 1000 + pg.current_ayah)) \
             END \
             FROM khatmil_juz_assignments a \
             JOIN khatmil_participants p ON p.id = a.participant_id \
             JOIN khatmil_campaigns c ON c.id = a.campaign_id \
             LEFT JOIN khatmil_progress pg ON pg.assignment_id = a.id \
             WHERE a.id = ?")
        .bind(assignment_id).fetch_optional(pool).await.map_err(dberr)?
        .ok_or_else(|| AppError::NotFound("assignment tidak ada".into()))?;
    Ok(AssignmentOut {
        id: r.0, campaign_id: r.1, campaign_name: r.2, juz: r.3, status: r.4, due_at: r.5,
        pages_read: r.6, minutes_read: r.7, verification: r.8,
        current_surah: r.9, current_ayah: r.10, read_ayat: r.12, juz_total_ayat: r.11,
        progress_pct: pct(r.12.unwrap_or(0), r.11),
    })
}

// =====================================================================
// POST progress v2 — POSISI-ONLY (rev 3.3, KEPUTUSAN OPSI B)
// Return (out, created) — created=false utk replay posisi sama (idempotent 200)
// =====================================================================

pub async fn post_progress(
    pool: &MySqlPool, user_id: i64, assignment_id: i64, req: ProgressReq,
) -> Result<(AssignmentOut, bool), AppError> {
    // posisi WAJIB
    let (surah, ayah) = match (req.current_surah, req.current_ayah) {
        (Some(s), Some(a)) => (s, a),
        _ => return Err(AppError::Unprocessable(
            "lapor posisi bacaan Surah/Ayat (current_surah & current_ayah) — format lama pages/minutes tidak lagi didukung".into())),
    };
    if !(1..=114).contains(&surah) || ayah < 1 {
        return Err(AppError::Unprocessable("current_surah 1-114 & current_ayah >= 1".into()));
    }
    // telemetri legacy (disimpan, TIDAK dipakai utk completion)
    let pages = req.pages_read.unwrap_or(0).clamp(0, 22);
    let minutes = req.minutes_read.unwrap_or(0).clamp(0, 1440);

    let row: (i64, i64, i64, String, i64, String) = sqlx::query_as(
        "SELECT a.id, a.campaign_id, a.juz, a.status, IFNULL(c.require_manual_verification, 0), c.name \
         FROM khatmil_juz_assignments a \
         JOIN khatmil_participants p ON p.id = a.participant_id \
         JOIN khatmil_campaigns c ON c.id = a.campaign_id \
         WHERE a.id = ? AND p.user_id = ?")
        .bind(assignment_id).bind(user_id)
        .fetch_optional(pool).await.map_err(dberr)?
        .ok_or_else(|| AppError::NotFound("assignment tidak ada / bukan milik anda".into()))?;
    let (aid, _cid, juz, status, manual, cname) = row;
    if status == "COMPLETED" {
        return Err(AppError::Conflict("juz sudah COMPLETED".into()));
    }

    // validasi rentang + kalkulasi (F1.3)
    ensure_position_in_juz(pool, juz, surah, ayah).await?;
    let total = juz_total_ayat(pool, juz).await?;
    let off = ayat_offset(pool, juz, surah, ayah).await?;
    let completed = off >= total && total > 0;

    // IDEMPOTENT: posisi sama dgn rollup saat ini -> 200 no-op tanpa event baru
    let curpos: Option<(Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT current_surah_id, current_ayah FROM khatmil_progress WHERE assignment_id = ?")
        .bind(aid).fetch_optional(pool).await.map_err(dberr)?;
    if let Some((Some(cs), Some(ca))) = curpos {
        if cs == surah && ca == ayah {
            let out = assignment_out(pool, aid).await?;
            return Ok((out, false));
        }
    }

    let mut tx = pool.begin().await.map_err(dberr)?;
    sqlx::query("INSERT INTO khatmil_progress_events (assignment_id, user_id, pages_read, minutes_read, current_surah_id, current_ayah, note) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(aid).bind(user_id).bind(pages).bind(minutes).bind(surah).bind(ayah).bind(&req.note)
        .execute(&mut *tx).await.map_err(dberr)?;
    // completion v2: posisi == ayat terakhir juz
    let new_status = if completed { "COMPLETED" } else { "IN_PROGRESS" };
    let verification = if completed {
        if manual != 0 { "SELF_REPORTED" } else { "SYSTEM_VERIFIED" }
    } else { "SELF_REPORTED" };
    sqlx::query(
        "INSERT INTO khatmil_progress (assignment_id, pages_read, minutes_read, verification, current_surah_id, current_ayah) \
         VALUES (?, ?, ?, ?, ?, ?) AS new \
         ON DUPLICATE KEY UPDATE pages_read = GREATEST(new.pages_read, khatmil_progress.pages_read), \
         minutes_read = GREATEST(new.minutes_read, khatmil_progress.minutes_read), \
         verification = new.verification, current_surah_id = new.current_surah_id, current_ayah = new.current_ayah")
        .bind(aid).bind(pages).bind(minutes).bind(verification).bind(surah).bind(ayah)
        .execute(&mut *tx).await.map_err(dberr)?;
    sqlx::query("UPDATE khatmil_juz_assignments SET status = ? WHERE id = ?")
        .bind(new_status).bind(aid)
        .execute(&mut *tx).await.map_err(dberr)?;
    if completed && manual == 0 {
        // SYSTEM_VERIFIED = validasi rule/posisi oleh sistem — BUKAN bukti fisik membaca (rev 3.3)
        sqlx::query("UPDATE khatmil_progress SET verified_at = UTC_TIMESTAMP() WHERE assignment_id = ? AND verified_at IS NULL")
            .bind(aid).execute(&mut *tx).await.map_err(dberr)?;
        sqlx::query("INSERT INTO user_notifications (user_id, template_code, title, body, data, channel) \
                     VALUES (?, 'KHOTMIL_JUZ_DONE', 'Juz selesai', ?, CAST(? AS JSON), 'IN_APP')")
            .bind(user_id)
            .bind(format!("Juz {juz} di campaign {cname} selesai — terverifikasi sistem (posisi mencapai ayat terakhir)"))
            .bind(format!("{{\"deeplink\":\"khatmil:assignment:{aid}\",\"juz\":{juz}}}"))
            .execute(&mut *tx).await.map_err(dberr)?;
    }
    sqlx::query("INSERT INTO activity_events (user_id, event_type, ref_type, ref_id, payload) \
                 VALUES (?, 'khatmil.progress', 'khatmil_assignment', ?, CAST(? AS JSON))")
        .bind(user_id).bind(aid.to_string())
        .bind(format!("{{\"juz\":{juz},\"surah\":{surah},\"ayah\":{ayah},\"read_ayat\":{off},\"juz_total_ayat\":{total},\"completed\":{completed}}}"))
        .execute(&mut *tx).await.map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;

    let out = assignment_out(pool, aid).await?;
    Ok((out, true))
}

// =====================================================================
// My assignments (+ posisi — mobile F0a)
// =====================================================================

pub async fn my_assignments(pool: &MySqlPool, user_id: i64) -> Result<Vec<AssignmentOut>, AppError> {
    let rows: Vec<(i64, i64, String, i64, String, Option<String>, i64, i64, String, Option<i64>, Option<i64>, i64, Option<i64>)> =
        sqlx::query_as(
            "SELECT a.id, a.campaign_id, c.name, a.juz, a.status, DATE_FORMAT(a.due_at, '%Y-%m-%dT%H:%i:%sZ'), \
             IFNULL(pg.pages_read, 0), IFNULL(pg.minutes_read, 0), IFNULL(pg.verification, 'PENDING'), \
             pg.current_surah_id, pg.current_ayah, \
             (SELECT COUNT(*) FROM quran_ayahs qa WHERE qa.juz = a.juz), \
             CASE WHEN pg.current_surah_id IS NOT NULL THEN \
               (SELECT COUNT(*) FROM quran_ayahs qb WHERE qb.juz = a.juz AND (qb.surah_id * 1000 + qb.ayah_number) <= (pg.current_surah_id * 1000 + pg.current_ayah)) \
             END \
             FROM khatmil_juz_assignments a \
             JOIN khatmil_participants p ON p.id = a.participant_id \
             JOIN khatmil_campaigns c ON c.id = a.campaign_id \
             LEFT JOIN khatmil_progress pg ON pg.assignment_id = a.id \
             WHERE p.user_id = ? AND c.status IN ('ACTIVE','SCHEDULED') AND a.status IN ('ASSIGNED','IN_PROGRESS') \
             ORDER BY a.campaign_id, a.juz")
        .bind(user_id).fetch_all(pool).await.map_err(dberr)?;
    Ok(rows.into_iter().map(|r| AssignmentOut {
        id: r.0, campaign_id: r.1, campaign_name: r.2, juz: r.3, status: r.4, due_at: r.5,
        pages_read: r.6, minutes_read: r.7, verification: r.8,
        current_surah: r.9, current_ayah: r.10, read_ayat: r.12, juz_total_ayat: r.11,
        progress_pct: pct(r.12.unwrap_or(0), r.11),
    }).collect())
}

// =====================================================================
// Participants v2 (rev 3.2/3.3) — per peserta + metrik
// =====================================================================

pub async fn participants(pool: &MySqlPool, campaign_id: i64) -> Result<Vec<ParticipantOut>, AppError> {
    let (target,): (i64,) = sqlx::query_as("SELECT target_khataman FROM khatmil_campaigns WHERE id = ?")
        .bind(campaign_id).fetch_optional(pool).await.map_err(dberr)?
        .ok_or_else(|| AppError::NotFound("campaign tidak ada".into()))?;

    // agregat per peserta (GROUP BY participant — join events hanya utk last_reported)
    let agg: Vec<(i64, String, String, i64, i64, i64, Option<String>)> = sqlx::query_as(
        "SELECT p.user_id, IFNULL(up.full_name, 'Tanpa nama'), DATE_FORMAT(p.joined_at, '%Y-%m-%dT%H:%i:%sZ'), \
         CAST(IFNULL(SUM(a.status = 'COMPLETED'), 0) AS SIGNED), \
         CAST(IFNULL(SUM(a.status IN ('ASSIGNED','IN_PROGRESS')), 0) AS SIGNED), \
         CAST(IFNULL(SUM(CASE WHEN a.status = 'COMPLETED' THEN IFNULL(pg.minutes_read, 0) ELSE 0 END), 0) AS SIGNED), \
         DATE_FORMAT((SELECT MAX(e.recorded_at) FROM khatmil_progress_events e JOIN khatmil_juz_assignments a2 ON a2.id = e.assignment_id WHERE a2.participant_id = p.id), '%Y-%m-%dT%H:%i:%sZ') \
         FROM khatmil_participants p \
         LEFT JOIN user_profiles up ON up.user_id = p.user_id \
         LEFT JOIN khatmil_juz_assignments a ON a.participant_id = p.id \
         LEFT JOIN khatmil_progress pg ON pg.assignment_id = a.id \
         WHERE p.campaign_id = ? \
         GROUP BY p.id, p.user_id, up.full_name, p.joined_at \
         ORDER BY 4 DESC, 2 ASC")
        .bind(campaign_id).fetch_all(pool).await.map_err(dberr)?;

    // rincian juz per peserta (aktif + selesai)
    let detail: Vec<(i64, i64, String, Option<i64>, Option<i64>, Option<String>, Option<i64>, i64)> = sqlx::query_as(
        "SELECT p.user_id, a.juz, a.status, pg.current_surah_id, pg.current_ayah, \
         DATE_FORMAT((SELECT MAX(e.recorded_at) FROM khatmil_progress_events e WHERE e.assignment_id = a.id), '%Y-%m-%dT%H:%i:%sZ'), \
         CASE WHEN pg.current_surah_id IS NOT NULL THEN \
           (SELECT COUNT(*) FROM quran_ayahs qb WHERE qb.juz = a.juz AND (qb.surah_id * 1000 + qb.ayah_number) <= (pg.current_surah_id * 1000 + pg.current_ayah)) \
         END, \
         (SELECT COUNT(*) FROM quran_ayahs qa WHERE qa.juz = a.juz) \
         FROM khatmil_participants p \
         JOIN khatmil_juz_assignments a ON a.participant_id = p.id \
         LEFT JOIN khatmil_progress pg ON pg.assignment_id = a.id \
         WHERE p.campaign_id = ? ORDER BY a.juz")
        .bind(campaign_id).fetch_all(pool).await.map_err(dberr)?;

    // completed_at per juz selesai (dari verified_at; fallback updated_at)
    let done_at: Vec<(i64, i64, Option<String>)> = sqlx::query_as(
        "SELECT p.user_id, a.juz, DATE_FORMAT(COALESCE(pg.verified_at, pg.updated_at), '%Y-%m-%dT%H:%i:%sZ') \
         FROM khatmil_participants p \
         JOIN khatmil_juz_assignments a ON a.participant_id = p.id AND a.status = 'COMPLETED' \
         LEFT JOIN khatmil_progress pg ON pg.assignment_id = a.id \
         WHERE p.campaign_id = ? ORDER BY a.juz")
        .bind(campaign_id).fetch_all(pool).await.map_err(dberr)?;
    let mut done_map: std::collections::HashMap<(i64, i64), Option<String>> = std::collections::HashMap::new();
    for (uid, juz, at) in done_at { done_map.insert((uid, juz), at); }

    let mut by_user: std::collections::HashMap<i64, Vec<(i64, String, Option<i64>, Option<i64>, Option<String>, Option<i64>, i64)>> =
        std::collections::HashMap::new();
    for (uid, juz, status, cs, ca, last, off, total) in detail {
        by_user.entry(uid).or_default().push((juz, status, cs, ca, last, off, total));
    }

    let mut out = Vec::with_capacity(agg.len());
    for (uid, full_name, joined, done_cnt, act_cnt, minutes_total, last_reported) in agg {
        let mut juz_active = Vec::new();
        let mut juz_done = Vec::new();
        if let Some(list) = by_user.get(&uid) {
            for (juz, status, cs, ca, last, off, total) in list.iter().cloned() {
                if status == "COMPLETED" {
                    juz_done.push(JuzDoneOut { juz, completed_at: done_map.get(&(uid, juz)).cloned().flatten() });
                } else {
                    juz_active.push(JuzActiveOut {
                        juz, status: status.clone(),
                        current_surah: cs, current_ayah: ca,
                        read_ayat: off, juz_total_ayat: total,
                        progress_pct: pct(off.unwrap_or(0), total),
                        last_reported_at: last.clone(),
                    });
                }
            }
        }
        let held = done_cnt + act_cnt;
        out.push(ParticipantOut {
            user_id: uid, full_name, joined_at: joined,
            juz_active, juz_done,
            juz_active_count: act_cnt, juz_done_count: done_cnt,
            minutes_total,
            task_progress_pct: if held > 0 { pct(done_cnt, held) } else { None },
            contribution_pct: if target > 0 { pct(done_cnt, 30 * target) } else { None },
            last_reported_at: last_reported,
        });
    }
    out.sort_by(|a, b| b.juz_done_count.cmp(&a.juz_done_count).then_with(|| a.full_name.cmp(&b.full_name)));
    Ok(out)
}

// =====================================================================
// Activity feed campaign (BARU rev 3.x) — 50 laporan terbaru
// =====================================================================

pub async fn activity(pool: &MySqlPool, campaign_id: i64, limit: i64) -> Result<Vec<ActivityEventOut>, AppError> {
    let rows: Vec<(String, i64, Option<i64>, Option<i64>, Option<String>, i64, String)> = sqlx::query_as(
        "SELECT IFNULL(up.full_name, '—'), a.juz, e.current_surah_id, e.current_ayah, e.note, \
         CASE WHEN e.current_surah_id IS NOT NULL AND (e.current_surah_id * 1000 + e.current_ayah) = (jz.end_surah_id * 1000 + jz.end_ayah) THEN 1 ELSE 0 END, \
         DATE_FORMAT(e.recorded_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM khatmil_progress_events e \
         JOIN khatmil_juz_assignments a ON a.id = e.assignment_id \
         JOIN khatmil_participants p ON p.id = a.participant_id \
         LEFT JOIN user_profiles up ON up.user_id = p.user_id \
         JOIN quran_juzs jz ON jz.id = a.juz \
         WHERE a.campaign_id = ? \
         ORDER BY e.id DESC LIMIT ?")
        .bind(campaign_id).bind(limit)
        .fetch_all(pool).await.map_err(dberr)?;
    Ok(rows.into_iter().map(|r| ActivityEventOut {
        full_name: r.0, juz: r.1, current_surah: r.2, current_ayah: r.3, note: r.4,
        completed: r.5 != 0, created_at: r.6,
    }).collect())
}

// =====================================================================
// Worker: pengingat juz mangkrak (F1.8) — dipanggil worker.rs
// =====================================================================

async fn setting_str(pool: &MySqlPool, key: &str) -> Option<String> {
    sqlx::query_scalar("SELECT CAST(value AS CHAR) FROM settings WHERE `key` = ?")
        .bind(key).fetch_optional(pool).await.ok().flatten()
}

async fn setting_i64(pool: &MySqlPool, key: &str, default: i64) -> i64 {
    setting_str(pool, key).await
        .and_then(|s| s.trim_matches('"').parse().ok())
        .unwrap_or(default)
}

pub async fn job_stale_reminder(pool: &MySqlPool) -> Result<u64, AppError> {
    let enabled = setting_str(pool, "khatmil_reminder_enabled").await
        .map(|s| s.trim_matches('"').eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !enabled { return Ok(0); }
    let stale_days = setting_i64(pool, "khatmil_reminder_stale_days", 3).await.clamp(0, 90);
    // assignment aktif di campaign ACTIVE yang mangkrak
    let rows: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "SELECT a.id, a.juz, p.user_id, c.name \
         FROM khatmil_juz_assignments a \
         JOIN khatmil_participants p ON p.id = a.participant_id \
         JOIN khatmil_campaigns c ON c.id = a.campaign_id \
         WHERE c.status = 'ACTIVE' AND a.status IN ('ASSIGNED','IN_PROGRESS') \
           AND IFNULL((SELECT MAX(e.recorded_at) FROM khatmil_progress_events e WHERE e.assignment_id = a.id), a.assigned_at) \
               < DATE_SUB(UTC_TIMESTAMP(), INTERVAL ? DAY)")
        .bind(stale_days)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut sent = 0u64;
    for (aid, juz, uid, cname) in rows {
        // anti-spam: skip bila sudah ada notif KHATMIL_JUZ_STALE utk assignment ini < 24 jam
        let recent: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM user_notifications \
             WHERE user_id = ? AND template_code = 'KHATMIL_JUZ_STALE' \
               AND created_at > DATE_SUB(UTC_TIMESTAMP(), INTERVAL 24 HOUR) \
               AND CAST(JSON_UNQUOTE(JSON_EXTRACT(data, '$.assignment_id')) AS SIGNED) = ?")
            .bind(uid).bind(aid)
            .fetch_one(pool).await.map_err(dberr)?;
        if recent.0 > 0 { continue; }
        sqlx::query("INSERT INTO user_notifications (user_id, template_code, title, body, data, channel) \
                     VALUES (?, 'KHATMIL_JUZ_STALE', 'Pengingat Juz', ?, CAST(? AS JSON), 'IN_APP')")
            .bind(uid)
            .bind(format!("Juz {juz} di campaign {cname} belum selesai — yuk lanjutkan bacaannya"))
            .bind(format!("{{\"deeplink\":\"khatmil:assignment:{aid}\",\"assignment_id\":{aid},\"juz\":{juz}}}"))
            .execute(pool).await.map_err(dberr)?;
        sent += 1;
    }
    Ok(sent)
}

pub fn ts(d: chrono::NaiveDateTime) -> String {
    d.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
