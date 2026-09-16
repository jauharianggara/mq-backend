//! Penugasan Pembina Khatmil (plan 2026-09-15_mq-khatmil-v2 rev 4 + sederhanakan-app rev 1).
//!
//! Dua langkah: admin isi `pending_ustadz_id` -> ustadz ACC di app -> `ustadz_id` terisi.
//! Saldo/assignment tidak tersentuh di sini. Monitoring pembina = read-only.

use sqlx::MySqlPool;

use crate::shared::error::AppError;

pub fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}

/// Pastikan baris kelompok 1..group_count tersedia untuk campaign (idempotent).
/// Dipanggil saat campaign dibuat & sebelum penugasan/monitoring campaign lama.
pub async fn ensure_groups(pool: &MySqlPool, campaign_id: i64) -> Result<(), AppError> {
    sqlx::query(
        "INSERT IGNORE INTO khatmil_groups (campaign_id, group_no) \
         SELECT c.id, g.n FROM khatmil_campaigns c \
         JOIN (SELECT 1 n UNION SELECT 2 UNION SELECT 3 UNION SELECT 4 UNION SELECT 5 UNION SELECT 6 \
               UNION SELECT 7 UNION SELECT 8 UNION SELECT 9 UNION SELECT 10) g \
         ON g.n <= c.group_count WHERE c.id = ?")
        .bind(campaign_id)
        .execute(pool)
        .await
        .map_err(dberr)?;
    Ok(())
}

async fn notify<'e, E>(ex: E, user_id: i64, code: &str, title: &str, body: &str, deeplink: &str) -> Result<(), AppError>
where
    E: sqlx::Executor<'e, Database = sqlx::MySql>,
{
    let data = format!("{{\"deeplink\":\"khatmil:{}\"}}", deeplink.replace('"', ""));
    sqlx::query(
        "INSERT INTO user_notifications (user_id, template_code, title, body, data, channel) \
         VALUES (?, ?, ?, ?, CAST(? AS JSON), 'IN_APP')")
        .bind(user_id).bind(code).bind(title).bind(body).bind(data)
        .execute(ex).await.map_err(dberr)?;
    Ok(())
}

async fn notify_admins(pool: &MySqlPool, title: &str, body: &str) {
    let ids: Vec<(i64,)> = sqlx::query_as(
        "SELECT DISTINCT ur.user_id FROM user_roles ur JOIN roles r ON r.id = ur.role_id \
         WHERE r.code IN ('ADMIN','SUPER_ADMIN') LIMIT 20")
        .fetch_all(pool).await.unwrap_or_default();
    for (uid,) in ids {
        let _ = notify(&mut *pool.acquire().await.unwrap(), uid,
            "KHATMIL_ASSIGN_RESULT", title, body, "").await;
    }
}

// ===================== ustadz =====================

/// GET /ustadz/khatmil — penugasan pending + khatmil dibina + khatmil aktif umum.
pub async fn overview(pool: &MySqlPool, ustadz_id: i64) -> Result<serde_json::Value, AppError> {
    let pending: Vec<(i64, i64, i64, String, i64, String)> = sqlx::query_as(
        "SELECT g.id, g.campaign_id, g.group_no, c.name, c.target_khataman, \
         DATE_FORMAT(g.pending_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM khatmil_groups g JOIN khatmil_campaigns c ON c.id = g.campaign_id \
         WHERE g.pending_ustadz_id = ? ORDER BY g.pending_at DESC LIMIT 20")
        .bind(ustadz_id).fetch_all(pool).await.map_err(dberr)?;
    let pending_out: Vec<serde_json::Value> = pending.iter().map(|g| serde_json::json!({
        "group_id": g.0, "campaign_id": g.1, "group_no": g.2,
        "campaign_name": g.3, "target_khataman": g.4, "requested_at": g.5,
    })).collect();

    let dibina: Vec<(i64, i64, i64, String, i64)> = sqlx::query_as(
        "SELECT g.id, g.campaign_id, g.group_no, c.name, c.target_khataman \
         FROM khatmil_groups g JOIN khatmil_campaigns c ON c.id = g.campaign_id \
         WHERE g.ustadz_id = ? AND c.status IN ('ACTIVE','COMPLETED') ORDER BY c.id DESC LIMIT 50")
        .bind(ustadz_id).fetch_all(pool).await.map_err(dberr)?;
    let mut dibina_out = Vec::new();
    for g in &dibina {
        let (filled, completed): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), \
             (SELECT COUNT(*) FROM khatmil_juz_assignments a WHERE a.group_id = ? AND a.status = 'COMPLETED') \
             FROM khatmil_juz_assignments a WHERE a.group_id = ?")
            .bind(g.0).bind(g.0).fetch_one(pool).await.map_err(dberr)?;
        dibina_out.push(serde_json::json!({
            "group_id": g.0, "campaign_id": g.1, "group_no": g.2,
            "campaign_name": g.3, "target_khataman": g.4,
            "filled": filled, "completed": completed,
            "khatam": filled == 30 && completed >= 30,
        }));
    }

    let aktif: Vec<(i64, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, name, status, DATE_FORMAT(period_start, '%Y-%m-%d'), DATE_FORMAT(period_end, '%Y-%m-%d') \
         FROM khatmil_campaigns WHERE status = 'ACTIVE' ORDER BY id DESC LIMIT 20")
        .fetch_all(pool).await.map_err(dberr)?;
    let aktif_out: Vec<serde_json::Value> = aktif.iter().map(|c| serde_json::json!({
        "id": c.0, "name": c.1, "status": c.2, "period_start": c.3, "period_end": c.4,
    })).collect();

    Ok(serde_json::json!({
        "pending": pending_out,
        "dibina": dibina_out,
        "aktif": aktif_out,
    }))
}

/// GET /ustadz/khatmil/{campaign_id} — peta 30 juz kelompok yang dibina (read-only).
pub async fn group_progress(pool: &MySqlPool, ustadz_id: i64, campaign_id: i64) -> Result<serde_json::Value, AppError> {
    let groups: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, group_no FROM khatmil_groups WHERE campaign_id = ? AND ustadz_id = ? ORDER BY group_no")
        .bind(campaign_id).bind(ustadz_id).fetch_all(pool).await.map_err(dberr)?;
    if groups.is_empty() {
        return Err(AppError::NotFound("Anda bukan pembina khatmil ini".into()));
    }
    let mut kelompok = Vec::new();
    for (gid, gno) in groups {
        let filled: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM khatmil_juz_assignments WHERE group_id = ?")
            .bind(gid).fetch_one(pool).await.map_err(dberr)?;
        let completed: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM khatmil_juz_assignments WHERE group_id = ? AND status = 'COMPLETED'")
            .bind(gid).fetch_one(pool).await.map_err(dberr)?;
        let juz_rows: Vec<(i64, Option<String>, Option<String>, Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT a.juz, a.status, \
             (SELECT COALESCE(NULLIF(p2.full_name,''),'-') FROM khatmil_participants pp LEFT JOIN user_profiles p2 ON p2.user_id = pp.user_id WHERE pp.id = a.participant_id), \
             pg.current_surah_id, pg.current_ayah \
             FROM khatmil_juz_assignments a \
             LEFT JOIN khatmil_progress pg ON pg.assignment_id = a.id \
             WHERE a.group_id = ? ORDER BY a.juz")
            .bind(gid).fetch_all(pool).await.map_err(dberr)?;
        let juzs: Vec<serde_json::Value> = juz_rows.iter().map(|j| serde_json::json!({
            "juz": j.0,
            "status": j.1,
            "pemilik": j.2.clone().unwrap_or_else(|| "-".into()),
            "posisi": j.3.map(|s| format!("QS {}:{}", s, j.4.unwrap_or(0))),
        })).collect();
        kelompok.push(serde_json::json!({
            "group_no": gno, "terisi": filled, "selesai": completed,
            "khatam": completed >= 30,
            "juz": juzs,
        }));
    }
    Ok(serde_json::json!({ "campaign_id": campaign_id, "kelompok": kelompok }))
}

/// POST /ustadz/khatmil-groups/{id}/accept — ACC penugasan (hanya ustadz yang ditunjuk).
pub async fn accept_group(pool: &MySqlPool, ustadz_id: i64, group_id: i64) -> Result<(), AppError> {
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT id, campaign_id, pending_ustadz_id FROM khatmil_groups \
         WHERE id = ? AND pending_ustadz_id = ?")
        .bind(group_id).bind(ustadz_id).fetch_optional(pool).await.map_err(dberr)?;
    let (gid, campaign_id, _pending) = row.ok_or_else(|| AppError::NotFound("penugasan tidak ditemukan".into()))?;
    let n = sqlx::query(
        "UPDATE khatmil_groups SET ustadz_id = pending_ustadz_id, pending_ustadz_id = NULL, pending_at = NULL \
         WHERE id = ? AND pending_ustadz_id = ?")
        .bind(gid).bind(ustadz_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Err(AppError::Conflict("penugasan sudah diproses".into()));
    }
    let cname: (String,) = sqlx::query_as("SELECT name FROM khatmil_campaigns WHERE id = ?")
        .bind(campaign_id).fetch_one(pool).await.map_err(dberr)?;
    notify_admins(pool, "Pembina khatmil diterima", &format!("Ustadz menerima penugasan Pembina \"{}\".", cname.0)).await;
    Ok(())
}

/// POST /ustadz/khatmil-groups/{id}/reject — tolak penugasan (kosong lagi utk admin).
pub async fn reject_group(pool: &MySqlPool, ustadz_id: i64, group_id: i64) -> Result<(), AppError> {
    let n = sqlx::query(
        "UPDATE khatmil_groups SET pending_ustadz_id = NULL, pending_at = NULL \
         WHERE id = ? AND pending_ustadz_id = ?")
        .bind(group_id).bind(ustadz_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Err(AppError::NotFound("penugasan tidak ditemukan".into()));
    }
    notify_admins(pool, "Penugasan pembina ditolak", "Ustadz menolak penugasan pembina — silakan tugaskan ustadz lain.").await;
    Ok(())
}

/// Admin: tugaskan pembina kelompok (dua langkah — menunggu ACC ustadz).
/// Menimpa penugasan pending sebelumnya. Ustadz harus ACTIVE + ber-role USTADZ.
pub async fn admin_assign(
    pool: &MySqlPool,
    _admin_id: i64,
    group_id: i64,
    ustadz_id: i64,
) -> Result<(), AppError> {
    let row: Option<(i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT g.campaign_id, g.group_no, g.pending_ustadz_id FROM khatmil_groups g \
         WHERE g.id = ?")
        .bind(group_id).fetch_optional(pool).await.map_err(dberr)?;
    let (campaign_id, group_no, prev_pending) =
        row.ok_or_else(|| AppError::NotFound("kelompok tidak ada".into()))?;

    let ustadz: Option<(String, String)> = sqlx::query_as(
        "SELECT u.status, COALESCE(NULLIF(up.full_name,''),'Ustadz') FROM users u \
         LEFT JOIN user_profiles up ON up.user_id = u.id WHERE u.id = ?")
        .bind(ustadz_id).fetch_optional(pool).await.map_err(dberr)?;
    let (status, nama) = ustadz.ok_or_else(|| AppError::NotFound("ustadz tidak ada".into()))?;
    if status != "ACTIVE" {
        return Err(AppError::Unprocessable("akun ustadz tidak aktif".into()));
    }
    let is_ustadz: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_roles ur JOIN roles r ON r.id = ur.role_id \
         WHERE ur.user_id = ? AND r.code = 'USTADZ'")
        .bind(ustadz_id).fetch_one(pool).await.map_err(dberr)?;
    if is_ustadz == 0 {
        return Err(AppError::Unprocessable("user bukan ber-role USTADZ".into()));
    }

    // cek campaign masih aktif
    let cstat: (String,) = sqlx::query_as(
        "SELECT status FROM khatmil_campaigns WHERE id = ?")
        .bind(campaign_id).fetch_one(pool).await.map_err(dberr)?;
    if cstat.0 != "ACTIVE" && cstat.0 != "SCHEDULED" {
        return Err(AppError::Conflict(format!("khatmil berstatus {}", cstat.0)));
    }

    let _ = prev_pending; // penugasan lama otomatis tergantikan
    let n = sqlx::query(
        "UPDATE khatmil_groups SET pending_ustadz_id = ?, pending_at = UTC_TIMESTAMP() \
         WHERE id = ?")
        .bind(ustadz_id).bind(group_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Err(AppError::NotFound("kelompok tidak ada".into()));
    }
    let cname: (String,) = sqlx::query_as("SELECT name FROM khatmil_campaigns WHERE id = ?")
        .bind(campaign_id).fetch_one(pool).await.map_err(dberr)?;
    notify(pool, ustadz_id, "KHATMIL_ASSIGN_REQUEST", "Penugasan Pembina Khatmil",
        &format!("Anda ditugaskan menjadi Pembina khatmil \"{}\" Kelompok {}. \
Buka menu Khatmil untuk menerima atau menolak.", cname.0, group_no),
        &format!("{}:{}", campaign_id, group_id)).await?;
    Ok(())
}
