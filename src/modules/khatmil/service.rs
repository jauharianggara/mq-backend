//! Service modul khatmil — claim juz = INSERT murni; unique marker-aktif jaminan final.
use sqlx::MySqlPool;

use crate::modules::khatmil::dto::*;
use crate::shared::error::AppError;

fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}

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

pub async fn update_campaign(pool: &MySqlPool, id: i64, req: CampaignUpsertReq) -> Result<(), AppError> {
    validate_upsert(&req)?;
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
    let (cid, slug, name, mode, status, target, minmin, rmv, participants, completed, pct, ps, pe) = base
        .ok_or_else(|| AppError::NotFound("campaign tidak ada".into()))?;
    // peta 30 juz
    let rows: Vec<(i64, Option<String>, Option<String>, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT j.j, a.status, \
         (SELECT p2.full_name FROM user_profiles p2 JOIN khatmil_participants pp ON pp.user_id = p2.user_id WHERE pp.id = a.participant_id), \
         pg.pages_read, pg.minutes_read \
         FROM (SELECT 1 j UNION SELECT 2 UNION SELECT 3 UNION SELECT 4 UNION SELECT 5 UNION SELECT 6 UNION SELECT 7 UNION SELECT 8 UNION SELECT 9 UNION SELECT 10 \
               UNION SELECT 11 UNION SELECT 12 UNION SELECT 13 UNION SELECT 14 UNION SELECT 15 UNION SELECT 16 UNION SELECT 17 UNION SELECT 18 UNION SELECT 19 UNION SELECT 20 \
               UNION SELECT 21 UNION SELECT 22 UNION SELECT 23 UNION SELECT 24 UNION SELECT 25 UNION SELECT 26 UNION SELECT 27 UNION SELECT 28 UNION SELECT 29 UNION SELECT 30) j \
         LEFT JOIN khatmil_juz_assignments a ON a.campaign_id = ? AND a.juz = j.j AND a.status IN ('ASSIGNED','IN_PROGRESS') \
         LEFT JOIN khatmil_progress pg ON pg.assignment_id = a.id \
         ORDER BY j.j")
        .bind(id).fetch_all(pool).await.map_err(dberr)?;
    let juz_map = rows.into_iter().map(|r| JuzSlot {
        juz: r.0, status: r.1, owner_name: r.2, pages_read: r.3, minutes_read: r.4,
    }).collect();
    Ok(CampaignDetail {
        campaign: CampaignOut {
            id: cid, slug, name, mode, status, target_khataman: target,
            min_minutes_per_juz: minmin, require_manual_verification: rmv != 0,
            participants, juz_completed: completed, progress_pct: pct,
        },
        juz_map, period_start: ps, period_end: pe,
    })
}

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
    let (status, minmin): (String, i64) = sqlx::query_as(
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
            // auto-assign: juz kosong terkecil
            let free: Option<(i64,)> = sqlx::query_as(
                "SELECT j.j FROM (SELECT 1 j UNION SELECT 2 UNION SELECT 3 UNION SELECT 4 UNION SELECT 5 UNION SELECT 6 UNION SELECT 7 \
                  UNION SELECT 8 UNION SELECT 9 UNION SELECT 10 UNION SELECT 11 UNION SELECT 12 UNION SELECT 13 UNION SELECT 14 UNION SELECT 15 \
                  UNION SELECT 16 UNION SELECT 17 UNION SELECT 18 UNION SELECT 19 UNION SELECT 20 UNION SELECT 21 UNION SELECT 22 UNION SELECT 23 \
                  UNION SELECT 24 UNION SELECT 25 UNION SELECT 26 UNION SELECT 27 UNION SELECT 28 UNION SELECT 29 UNION SELECT 30) j \
                 WHERE NOT EXISTS (SELECT 1 FROM khatmil_juz_assignments a WHERE a.campaign_id = ? AND a.juz = j.j AND a.status IN ('ASSIGNED','IN_PROGRESS')) \
                 ORDER BY j.j LIMIT 1")
                .bind(campaign_id).fetch_optional(pool).await.map_err(dberr)?;
            free.map(|f| f.0).ok_or_else(|| AppError::Conflict("semua juz sudah diklaim".into()))?
        }
    };

    // CLAIM = INSERT murni — partial-unique marker-aktif (generated col) = jaminan final (0009)
    let ins = sqlx::query(
        "INSERT INTO khatmil_juz_assignments (campaign_id, participant_id, juz) VALUES (?, ?, ?)")
        .bind(campaign_id).bind(participant.0).bind(juz)
        .execute(pool).await;
    match ins {
        Ok(_) => {}
        Err(sqlx::Error::Database(d)) if d.message().contains("khatmil_juz_active_uq") || d.message().contains("Duplicate entry") => {
            return Err(AppError::Conflict(format!("juz {juz} sudah diklaim peserta lain")));
        }
        Err(e) => return Err(dberr(e)),
    }
    let aid = sqlx::query_scalar::<_, i64>("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
        .fetch_one(pool).await.map_err(dberr)?;
    sqlx::query("INSERT IGNORE INTO khatmil_progress (assignment_id) VALUES (?)")
        .bind(aid).execute(pool).await.map_err(dberr)?;
    let name: (String,) = sqlx::query_as("SELECT name FROM khatmil_campaigns WHERE id = ?")
        .bind(campaign_id).fetch_one(pool).await.map_err(dberr)?;
    Ok(AssignmentOut {
        id: aid, campaign_id, campaign_name: name.0, juz, status: "ASSIGNED".into(),
        due_at: None, pages_read: 0, minutes_read: 0, verification: "PENDING".into(),
    })
}

pub async fn post_progress(
    pool: &MySqlPool, user_id: i64, assignment_id: i64, req: ProgressReq,
) -> Result<AssignmentOut, AppError> {
    if !(1..=22).contains(&req.pages_read) {
        return Err(AppError::Unprocessable("pages_read 1-22".into()));
    }
    if !(1..=1440).contains(&req.minutes_read) {
        return Err(AppError::Unprocessable("minutes_read 1-1440".into()));
    }
    let row: (i64, i64, i64, String, i64, i64, String) = sqlx::query_as(
        "SELECT a.id, a.campaign_id, a.juz, a.status, c.min_minutes_per_juz, IFNULL(c.require_manual_verification, 0), c.name \
         FROM khatmil_juz_assignments a \
         JOIN khatmil_participants p ON p.id = a.participant_id \
         JOIN khatmil_campaigns c ON c.id = a.campaign_id \
         WHERE a.id = ? AND p.user_id = ?")
        .bind(assignment_id).bind(user_id)
        .fetch_optional(pool).await.map_err(dberr)?
        .ok_or_else(|| AppError::NotFound("assignment tidak ada / bukan milik anda".into()))?;
    let (aid, campaign_id, juz, status, minmin, manual, cname) = row;
    if status == "COMPLETED" {
        return Err(AppError::Conflict("juz sudah COMPLETED".into()));
    }

    let mut tx = pool.begin().await.map_err(dberr)?;
    sqlx::query("INSERT INTO khatmil_progress_events (assignment_id, user_id, pages_read, minutes_read, note) \
                 VALUES (?, ?, ?, ?, ?)")
        .bind(aid).bind(user_id).bind(req.pages_read).bind(req.minutes_read).bind(&req.note)
        .execute(&mut *tx).await.map_err(dberr)?;
    // rule 0009: pages>=20 && minutes>=min -> COMPLETED (+verifikasi sesuai policy)
    let rule_ok = req.pages_read >= 20 && req.minutes_read >= minmin;
    let new_status = if rule_ok { "COMPLETED" } else { "IN_PROGRESS" };
    let verification = if rule_ok {
        if manual != 0 { "SELF_REPORTED" } else { "SYSTEM_VERIFIED" } // MANUAL_VERIFIED via ustadz (Phase 2)
    } else { "SELF_REPORTED" };
    sqlx::query(
        "INSERT INTO khatmil_progress (assignment_id, pages_read, minutes_read, verification) \
         VALUES (?, ?, ?, ?) AS new \
         ON DUPLICATE KEY UPDATE pages_read = GREATEST(new.pages_read, khatmil_progress.pages_read), \
         minutes_read = GREATEST(new.minutes_read, khatmil_progress.minutes_read), verification = new.verification")
        .bind(aid).bind(req.pages_read).bind(req.minutes_read).bind(verification)
        .execute(&mut *tx).await.map_err(dberr)?;
    sqlx::query("UPDATE khatmil_juz_assignments SET status = ? WHERE id = ?")
        .bind(new_status).bind(aid)
        .execute(&mut *tx).await.map_err(dberr)?;
    if rule_ok && manual == 0 {
        // auto-verified => verified_at terisi (definisi 0009)
        sqlx::query("UPDATE khatmil_progress SET verified_at = UTC_TIMESTAMP() WHERE assignment_id = ? AND verified_at IS NULL")
            .bind(aid).execute(&mut *tx).await.map_err(dberr)?;
        sqlx::query("INSERT INTO user_notifications (user_id, template_code, title, body, data, channel) \
                     VALUES (?, 'KHOTMIL_JUZ_DONE', 'Juz selesai', ?, CAST(? AS JSON), 'IN_APP')")
            .bind(user_id)
            .bind(format!("Juz {juz} di campaign {cname} selesai — terverifikasi sistem"))
            .bind(format!("{{\"deeplink\":\"khatmil:assignment:{aid}\"}}"))
            .execute(&mut *tx).await.map_err(dberr)?;
    }
    sqlx::query("INSERT INTO activity_events (user_id, event_type, ref_type, ref_id, payload) \
                 VALUES (?, 'khatmil.progress', 'khatmil_assignment', ?, CAST(? AS JSON))")
        .bind(user_id).bind(aid.to_string())
        .bind(format!("{{\"juz\":{juz},\"pages\":{},\"minutes\":{},\"completed\":{}}}", req.pages_read, req.minutes_read, rule_ok))
        .execute(&mut *tx).await.map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;

    let (pages, minutes, ver): (i64, i64, String) = sqlx::query_as(
        "SELECT pages_read, minutes_read, verification FROM khatmil_progress WHERE assignment_id = ?")
        .bind(aid).fetch_one(pool).await.map_err(dberr)?;
    Ok(AssignmentOut {
        id: aid, campaign_id, campaign_name: cname, juz, status: new_status.into(),
        due_at: None, pages_read: pages, minutes_read: minutes, verification: ver,
    })
}

pub async fn my_assignments(pool: &MySqlPool, user_id: i64) -> Result<Vec<AssignmentOut>, AppError> {
    let rows: Vec<(i64, i64, String, i64, String, Option<String>, i64, i64, String)> = sqlx::query_as(
        "SELECT a.id, a.campaign_id, c.name, a.juz, a.status, DATE_FORMAT(a.due_at, '%Y-%m-%dT%H:%i:%sZ'), \
         IFNULL(pg.pages_read, 0), IFNULL(pg.minutes_read, 0), IFNULL(pg.verification, 'PENDING') \
         FROM khatmil_juz_assignments a \
         JOIN khatmil_participants p ON p.id = a.participant_id \
         JOIN khatmil_campaigns c ON c.id = a.campaign_id \
         LEFT JOIN khatmil_progress pg ON pg.assignment_id = a.id \
         WHERE p.user_id = ? AND c.status IN ('ACTIVE','SCHEDULED') AND a.status IN ('ASSIGNED','IN_PROGRESS') \
         ORDER BY a.campaign_id, a.juz")
        .bind(user_id).fetch_all(pool).await.map_err(dberr)?;
    Ok(rows.into_iter().map(|r| AssignmentOut {
        id: r.0, campaign_id: r.1, campaign_name: r.2, juz: r.3, status: r.4,
        due_at: r.5, pages_read: r.6, minutes_read: r.7, verification: r.8,
    }).collect())
}

pub async fn participants(pool: &MySqlPool, campaign_id: i64) -> Result<Vec<(i64, String, i64, String)>, AppError> {
    let rows = sqlx::query_as::<_, (i64, String, i64, String)>(
        "SELECT p.id, IFNULL(up.full_name, 'Tanpa nama'), p.user_id, DATE_FORMAT(p.joined_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM khatmil_participants p LEFT JOIN user_profiles up ON up.user_id = p.user_id \
         WHERE p.campaign_id = ? ORDER BY p.id")
        .bind(campaign_id).fetch_all(pool).await.map_err(dberr)?;
    Ok(rows)
}
