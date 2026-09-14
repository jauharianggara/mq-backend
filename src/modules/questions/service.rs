//! Service modul questions — state machine (v11):
//! QUEUED → ASSIGNED → ANSWERED → PUBLISH_REQUESTED → PUBLISHED | REJECTED (+CLOSED)
use sqlx::MySqlPool;

use crate::infrastructure::storage::Storage;
use crate::modules::questions::dto::*;
use crate::shared::error::AppError;

fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}

const PRESIGN_SECS: u64 = 15 * 60;

pub async fn categories(pool: &MySqlPool) -> Result<Vec<CategoryOut>, AppError> {
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, slug, name FROM question_categories WHERE is_active = 1 ORDER BY sort_order")
        .fetch_all(pool).await.map_err(dberr)?;
    Ok(rows.into_iter().map(|r| CategoryOut { id: r.0, slug: r.1, name: r.2 }).collect())
}

/// catat transisi status (audit + SLA timestamps)
async fn transition<'e>(pool: impl sqlx::Executor<'e, Database = sqlx::MySql>, qid: i64, from: Option<&str>, to: &str, by: Option<i64>, note: Option<&str>) -> Result<(), AppError> {
    sqlx::query("INSERT INTO question_status_history (question_id, from_status, to_status, changed_by, note) VALUES (?, ?, ?, ?, ?)")
        .bind(qid).bind(from).bind(to).bind(by).bind(note)
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}

pub async fn create(
    pool: &MySqlPool, user_id: i64, req: CreateQuestionReq, client_key: Option<String>,
) -> Result<(QuestionThread, bool), AppError> {
    if req.title.trim().len() < 5 || req.title.len() > 200 {
        return Err(AppError::Unprocessable("title 5-200 karakter".into()));
    }
    let key = client_key.ok_or_else(|| AppError::Unprocessable("header Idempotency-Key wajib".into()))?;
    if key.len() > 64 {
        return Err(AppError::Unprocessable("Idempotency-Key max 64".into()));
    }
    // replay check dulu (pola memorization)
    if let Some(qid) = find_by_client_key(pool, user_id, &key).await? {
        let t = thread(pool, None, qid, false, true).await?; // pemilik view
        return Ok((q_from_thread(pool, &t), false));
    }
    let cat: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, name FROM question_categories WHERE id = ? AND is_active = 1")
        .bind(req.category_id).fetch_optional(pool).await.map_err(dberr)?;
    let (cat_id, cat_name) = cat.ok_or_else(|| AppError::Unprocessable("category_id tidak valid".into()))?;

    // auto-assign least-load per kategori (ustadz accepting + verified + ACTIVE + spec match)
    let ustadz: Option<(i64,)> = sqlx::query_as(
        "SELECT u.user_id FROM ustadz_profiles u \
         JOIN users us ON us.id = u.user_id AND us.status = 'ACTIVE' \
         JOIN ustadz_specializations sp ON sp.ustadz_id = u.user_id AND sp.category_id = ? \
         WHERE u.is_accepting_questions = 1 AND u.verified_at IS NOT NULL \
         ORDER BY (SELECT COUNT(*) FROM question_assignments qa \
                   JOIN questions q2 ON q2.id = qa.question_id \
                   WHERE qa.ustadz_id = u.user_id AND qa.status = 'ASSIGNED' \
                     AND q2.status IN ('QUEUED','ASSIGNED')) ASC, RAND() LIMIT 1")
        .bind(req.category_id).fetch_optional(pool).await.map_err(dberr)?;

    let mut tx = pool.begin().await.map_err(dberr)?;
    let ins = sqlx::query(
        "INSERT INTO questions (user_id, category_id, is_anonymous, title, body, status) \
         VALUES (?, ?, ?, ?, ?, 'QUEUED')")
        .bind(user_id).bind(cat_id).bind(req.is_anonymous).bind(req.title.trim()).bind(&req.body)
        .execute(&mut *tx).await.map_err(dberr)?;
    let qid = ins.last_insert_id() as i64;
    // idempotency question = client_key di PESAN PERTAMA (0015: question_messages)
    sqlx::query("INSERT INTO question_messages (question_id, sender_id, type, content, client_key) VALUES (?, ?, 'TEXT', ?, ?)")
        .bind(qid).bind(user_id).bind(req.body.as_deref().or(Some(req.title.trim())).unwrap_or("")).bind(&key)
        .execute(&mut *tx).await.map_err(dberr)?;
    transition(&mut *tx, qid, None, "QUEUED", Some(user_id), None).await?;
    let mut assigned = false;
    if let Some((uid,)) = ustadz {
        sqlx::query("INSERT INTO question_assignments (question_id, ustadz_id) VALUES (?, ?)")
            .bind(qid).bind(uid).execute(&mut *tx).await.map_err(dberr)?;
        sqlx::query("UPDATE questions SET status = 'ASSIGNED' WHERE id = ?").bind(qid)
            .execute(&mut *tx).await.map_err(dberr)?;
        transition(&mut *tx, qid, Some("QUEUED"), "ASSIGNED", None, Some("auto least-load")).await?;
        sqlx::query("INSERT INTO user_notifications (user_id, template_code, title, body, data, channel) \
                     VALUES (?, 'QUESTION_NEW', 'Pertanyaan baru', ?, CAST(? AS JSON), 'IN_APP')")
            .bind(uid)
            .bind(format!("Pertanyaan kategori {cat_name}: {}", req.title.trim()))
            .bind(format!("{{\"deeplink\":\"question:{qid}\"}}"))
            .execute(&mut *tx).await.map_err(dberr)?;
        assigned = true;
    }
    sqlx::query("INSERT INTO activity_events (user_id, event_type, ref_type, ref_id, payload) \
                 VALUES (?, 'question.asked', 'question', ?, CAST(? AS JSON))")
        .bind(user_id).bind(qid.to_string())
        .bind(format!("{{\"category\":\"{cat_name}\",\"assigned\":{assigned}}}"))
        .execute(&mut *tx).await.map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;
    let t = thread(pool, None, qid, false, true).await?;
    Ok((q_from_thread(pool, &t), true))
}

async fn find_by_client_key(pool: &MySqlPool, user_id: i64, key: &str) -> Result<Option<i64>, AppError> {
    // client_key questions disimpan di question_messages (0015: sender_id+client_key)
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT m.question_id FROM question_messages m WHERE m.sender_id = ? AND m.client_key = ? LIMIT 1")
        .bind(user_id).bind(key).fetch_optional(pool).await.map_err(dberr)?;
    Ok(row.map(|r| r.0))
}

fn q_from_thread(_pool: &MySqlPool, t: &QuestionThread) -> QuestionThread {
    QuestionThread { question: t.question.clone(), messages: t.messages.clone() }
}

type QRow = (i64, i64, String, i8, i64, String, String, Option<String>, String, Option<i64>, String, Option<String>, Option<String>);

async fn fetch_q(pool: &MySqlPool, qid: i64) -> Result<QRow, AppError> {
    sqlx::query_as(
        "SELECT q.id, q.user_id, IFNULL(p.full_name, 'Tanpa nama'), q.is_anonymous, q.category_id, c.name, q.title, q.body, q.status, \
         (SELECT qa.ustadz_id FROM question_assignments qa WHERE qa.question_id = q.id AND qa.status = 'ASSIGNED' ORDER BY qa.id DESC LIMIT 1), \
         DATE_FORMAT(q.created_at, '%Y-%m-%dT%H:%i:%sZ'), \
         DATE_FORMAT((SELECT MAX(h.created_at) FROM question_status_history h WHERE h.question_id = q.id AND h.to_status = 'ANSWERED'), '%Y-%m-%dT%H:%i:%sZ'), \
         DATE_FORMAT(q.published_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM questions q JOIN question_categories c ON c.id = q.category_id \
         LEFT JOIN user_profiles p ON p.user_id = q.user_id WHERE q.id = ?")
        .bind(qid).fetch_optional(pool).await.map_err(dberr)?
        .ok_or_else(|| AppError::NotFound("pertanyaan tidak ditemukan".into()))
}

/// ACL: pemilik / ustadz assigned / question.moderate / users.read.
fn access(_pool: &MySqlPool, viewer: i64, can_moderate: bool, can_admin: bool, row: &QRow) -> bool {
    row.1 == viewer || can_moderate || can_admin || row.9 == Some(viewer)
}

pub async fn thread(
    pool: &MySqlPool, storage: Option<&Storage>, qid: i64,
    can_moderate: bool, is_owner: bool,
) -> Result<QuestionThread, AppError> {
    let row = fetch_q(pool, qid).await?;
    let msgs: Vec<(i64, i64, Option<String>, i8, String, Option<String>, Option<i64>, Option<i32>, String)> = sqlx::query_as(
        "SELECT m.id, m.sender_id, IFNULL(p.full_name, 'Tanpa nama'), IFNULL(ua.is_ustadz, 0), m.type, m.content, m.media_id, m.duration_ms, \
         DATE_FORMAT(m.created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM question_messages m LEFT JOIN user_profiles p ON p.user_id = m.sender_id \
         LEFT JOIN (SELECT user_id, 1 is_ustadz FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE r.code = 'USTADZ') ua ON ua.user_id = m.sender_id \
         WHERE m.question_id = ? ORDER BY m.id")
        .bind(qid).fetch_all(pool).await.map_err(dberr)?;
    let mut out_msgs = Vec::new();
    for m in msgs {
        let url = match (storage, m.6) {
            (Some(s), Some(mid)) => presign_media(pool, s, mid).await.ok(),
            _ => None,
        };
        out_msgs.push(MessageOut {
            id: m.0, sender_id: m.1, sender_name: m.2, is_ustadz: m.3 != 0,
            type_: m.4, content: m.5, media_id: m.6, media_presigned_url: url,
            duration_ms: m.7, created_at: m.8,
        });
    }
    let assigned_name: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT p.full_name FROM question_assignments qa \
         LEFT JOIN user_profiles p ON p.user_id = qa.ustadz_id \
         WHERE qa.question_id = ? AND qa.status = 'ASSIGNED' ORDER BY qa.id DESC LIMIT 1")
        .bind(qid).fetch_optional(pool).await.map_err(dberr)?;
    let hide_asker = row.3 != 0 && !is_owner && !can_moderate;
    Ok(QuestionThread {
        question: QuestionOut {
            id: row.0,
            asker_name: if hide_asker { None } else { Some(row.2.clone()) },
            category_id: row.4, category_name: row.5, title: row.6, body: row.7, status: row.8,
            is_anonymous: row.3 != 0,
            assigned_ustadz_name: assigned_name.and_then(|n| n.0),
            created_at: row.10, answered_at: row.11, published_at: row.12,
        },
        messages: out_msgs,
    })
}

async fn presign_media(pool: &MySqlPool, storage: &Storage, media_id: i64) -> Result<String, AppError> {
    let (key,): (String,) = sqlx::query_as("SELECT storage_key FROM media WHERE id = ? AND status = 'READY'")
        .bind(media_id).fetch_one(pool).await.map_err(dberr)?;
    storage.presign_get(&key, PRESIGN_SECS).await
        .map_err(|e| { tracing::error!("{e}"); AppError::Internal("presign".into()) })
}

pub async fn my_questions(pool: &MySqlPool, user_id: i64, cursor: Option<i64>, limit: usize)
    -> Result<(Vec<QuestionOut>, Option<String>, bool), AppError> {
    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT q.id FROM questions q WHERE q.user_id = ? AND (? IS NULL OR q.id < ?) ORDER BY q.id DESC LIMIT ?")
        .bind(user_id).bind(cursor).bind(cursor).bind((limit + 1) as i64)
        .fetch_all(pool).await.map_err(dberr)?;
    let has_more = rows.len() > limit;
    let mut out = Vec::new();
    for qid in rows.iter().take(limit) {
        let t = thread(pool, None, *qid, false, true).await?;
        out.push(t.question);
    }
    let next = if has_more { out.last().map(|q| q.id.to_string()) } else { None };
    Ok((out, next, has_more))
}

pub async fn inbox(pool: &MySqlPool, ustadz_id: i64, cursor: Option<i64>, limit: usize)
    -> Result<(Vec<QuestionOut>, Option<String>, bool), AppError> {
    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT q.id FROM questions q JOIN question_assignments qa ON qa.question_id = q.id \
         WHERE qa.ustadz_id = ? AND qa.status = 'ASSIGNED' AND q.status IN ('QUEUED','ASSIGNED') \
         AND (? IS NULL OR q.id < ?) ORDER BY q.id ASC LIMIT ?")
        .bind(ustadz_id).bind(cursor).bind(cursor).bind((limit + 1) as i64)
        .fetch_all(pool).await.map_err(dberr)?;
    let has_more = rows.len() > limit;
    let mut out = Vec::new();
    for qid in rows.iter().take(limit) {
        let t = thread(pool, None, *qid, false, false).await?;
        out.push(t.question);
    }
    let next = if has_more { out.last().map(|q| q.id.to_string()) } else { None };
    Ok((out, next, has_more))
}

/// Arsip publik (PUBLISHED) — search FULLTEXT + fallback LIKE; disclaimer tampil klien.
pub async fn archive(pool: &MySqlPool, q: Option<String>, category: Option<String>, cursor: Option<i64>, limit: usize)
    -> Result<(Vec<QuestionOut>, Option<String>, bool), AppError> {
    let like = q.clone().map(|s| format!("%{s}%"));
    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT q.id FROM questions q JOIN question_categories c ON c.id = q.category_id \
         WHERE q.status = 'PUBLISHED' AND (? IS NULL OR q.id < ?) AND (? IS NULL OR c.slug = ?) \
           AND (? IS NULL OR MATCH(q.title, q.body) AGAINST(? IN NATURAL LANGUAGE MODE) OR q.title LIKE ? OR q.body LIKE ?) \
         ORDER BY q.published_at DESC, q.id DESC LIMIT ?")
        .bind(cursor).bind(cursor).bind(category.as_deref()).bind(category.as_deref())
        .bind(q.as_deref()).bind(q.as_deref()).bind(like.as_deref()).bind(like.as_deref())
        .bind((limit + 1) as i64)
        .fetch_all(pool).await.map_err(dberr)?;
    let has_more = rows.len() > limit;
    let mut out = Vec::new();
    for qid in rows.iter().take(limit) {
        let t = thread(pool, None, *qid, false, false).await?;
        out.push(t.question);
    }
    let next = if has_more { out.last().map(|q| q.id.to_string()) } else { None };
    Ok((out, next, has_more))
}

pub async fn moderation_queue(pool: &MySqlPool, status: Option<String>) -> Result<Vec<QuestionOut>, AppError> {
    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM questions WHERE (? IS NULL OR status = ?) AND status IN ('PUBLISH_REQUESTED','QUEUED') \
         ORDER BY FIELD(status, 'PUBLISH_REQUESTED', 'QUEUED'), id ASC LIMIT 100")
        .bind(status.as_deref()).bind(status.as_deref())
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    for qid in rows {
        let t = thread(pool, None, qid, true, false).await?;
        out.push(t.question);
    }
    Ok(out)
}

pub async fn send_message(
    pool: &MySqlPool, storage_needed: bool, sender: i64, qid: i64, req: SendMessageReq, client_key: Option<String>,
) -> Result<(MessageOut, bool), AppError> {
    if !matches!(req.type_.as_str(), "TEXT" | "VOICE" | "IMAGE" | "FILE") {
        return Err(AppError::Unprocessable("type TEXT/VOICE/IMAGE/FILE".into()));
    }
    if req.type_ == "TEXT" && req.content.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Err(AppError::Unprocessable("content wajib utk TEXT".into()));
    }
    if req.type_ != "TEXT" {
        if req.media_id.is_none() {
            return Err(AppError::Unprocessable(format!("media_id wajib utk {}", req.type_)));
        }
        if storage_needed {
            let mid = req.media_id.unwrap();
            let ok: Option<(i64,)> = sqlx::query_as(
                "SELECT id FROM media WHERE id = ? AND owner_id = ? AND status = 'READY'")
                .bind(mid).bind(sender).fetch_optional(pool).await.map_err(dberr)?;
            if ok.is_none() {
                return Err(AppError::Unprocessable("media_id tidak valid (READY milik anda)".into()));
            }
        }
    }
    let row = fetch_q(pool, qid).await?;
    let is_owner = row.1 == sender;
    let is_ustadz_assigned = row.9 == Some(sender);
    if !is_owner && !is_ustadz_assigned {
        return Err(AppError::Forbidden("bukan peserta thread".into()));
    }
    if !matches!(row.8.as_str(), "QUEUED" | "ASSIGNED" | "ANSWERED" | "PUBLISH_REQUESTED") {
        return Err(AppError::Conflict(format!("thread berstatus {} — dikunci", row.8)));
    }
    if let Some(key) = &client_key {
        if let Some(q2) = find_by_client_key(pool, sender, key).await? {
            if q2 == qid {
                // replay: ambil pesan existing by key
                let m: Option<(i64, i64, Option<String>, i8, String, Option<String>, Option<i64>, Option<i32>, String)> = sqlx::query_as(
                    "SELECT m.id, m.sender_id, IFNULL(p.full_name,'Tanpa nama'), 0, m.type, m.content, m.media_id, m.duration_ms, DATE_FORMAT(m.created_at,'%Y-%m-%dT%H:%i:%sZ') \
                     FROM question_messages m LEFT JOIN user_profiles p ON p.user_id = m.sender_id WHERE m.sender_id = ? AND m.client_key = ? LIMIT 1")
                    .bind(sender).bind(key).fetch_optional(pool).await.map_err(dberr)?;
                if let Some(r) = m {
                    return Ok((MessageOut {
                        id: r.0, sender_id: r.1, sender_name: r.2, is_ustadz: r.3 != 0,
                        type_: r.4, content: r.5, media_id: r.6, media_presigned_url: None, duration_ms: r.7, created_at: r.8,
                    }, false));
                }
            }
        }
    }
    let ins = sqlx::query(
        "INSERT INTO question_messages (question_id, sender_id, type, content, media_id, duration_ms, client_key) \
         VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(qid).bind(sender).bind(&req.type_).bind(&req.content).bind(req.media_id).bind(req.duration_ms)
        .bind(client_key.as_deref())
        .execute(pool).await.map_err(dberr)?;
    let is_ustadz = !is_owner;
    let (name,): (Option<String>,) = sqlx::query_as("SELECT full_name FROM user_profiles WHERE user_id = ?")
        .bind(sender).fetch_one(pool).await.map_err(dberr)?;
    Ok((MessageOut {
        id: ins.last_insert_id() as i64, sender_id: sender, sender_name: name, is_ustadz,
        type_: req.type_, content: req.content, media_id: req.media_id, media_presigned_url: None,
        duration_ms: req.duration_ms,
        created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    }, true))
}

pub async fn close(pool: &MySqlPool, user: i64, qid: i64) -> Result<(), AppError> {
    let row = fetch_q(pool, qid).await?;
    if row.1 != user {
        return Err(AppError::Forbidden("hanya penanya bisa menutup".into()));
    }
    if row.8 == "PUBLISHED" {
        return Err(AppError::Conflict("sudah PUBLISHED — tidak bisa ditutup".into()));
    }
    let from = row.8.clone();
    sqlx::query("UPDATE questions SET status = 'CLOSED', closed_at = UTC_TIMESTAMP() WHERE id = ?")
        .bind(qid).execute(pool).await.map_err(dberr)?;
    transition(pool, qid, Some(&from), "CLOSED", Some(user), None).await
}

pub async fn answer(pool: &MySqlPool, ustadz: i64, qid: i64) -> Result<(), AppError> {
    let row = fetch_q(pool, qid).await?;
    if row.9 != Some(ustadz) {
        return Err(AppError::Forbidden("bukan ustadz yang ditugaskan".into()));
    }
    if !matches!(row.8.as_str(), "QUEUED" | "ASSIGNED") {
        return Err(AppError::Conflict(format!("status {} — tidak bisa di-answer", row.8)));
    }
    sqlx::query("UPDATE questions SET status = 'ANSWERED', updated_at = UTC_TIMESTAMP() WHERE id = ?")
        .bind(qid).execute(pool).await.map_err(dberr)?;
    sqlx::query("UPDATE question_assignments SET responded_at = UTC_TIMESTAMP() WHERE question_id = ? AND ustadz_id = ? AND status = 'ASSIGNED'")
        .bind(qid).bind(ustadz).execute(pool).await.map_err(dberr)?;
    transition(pool, qid, Some(&row.8), "ANSWERED", Some(ustadz), None).await?;
    // notif penanya
    sqlx::query("INSERT INTO user_notifications (user_id, template_code, title, body, data, channel) \
                 VALUES (?, 'QUESTION_ANSWERED', 'Pertanyaan dijawab', 'Pertanyaan Anda telah dijawab ustadz', CAST(? AS JSON), 'IN_APP')")
        .bind(row.1).bind(format!("{{\"deeplink\":\"question:{qid}\"}}"))
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}

pub async fn publish_request(pool: &MySqlPool, ustadz: i64, qid: i64) -> Result<(), AppError> {
    let row = fetch_q(pool, qid).await?;
    if row.9 != Some(ustadz) {
        return Err(AppError::Forbidden("bukan ustadz yang ditugaskan".into()));
    }
    if row.8 != "ANSWERED" {
        return Err(AppError::Conflict(format!("harus ANSWERED dulu (sekarang {})", row.8)));
    }
    sqlx::query("UPDATE questions SET status = 'PUBLISH_REQUESTED' WHERE id = ?")
        .bind(qid).execute(pool).await.map_err(dberr)?;
    transition(pool, qid, Some("ANSWERED"), "PUBLISH_REQUESTED", Some(ustadz), None).await
}

/// Approve oleh moderator/admin (question.publish.moderate) — disclaimer flag di payload klien.
pub async fn publish(pool: &MySqlPool, approver: i64, qid: i64) -> Result<(), AppError> {
    let row = fetch_q(pool, qid).await?;
    if row.8 != "PUBLISH_REQUESTED" {
        return Err(AppError::Conflict(format!("harus PUBLISH_REQUESTED (sekarang {})", row.8)));
    }
    // published_message = pesan ustadz terakhir
    let last: Option<(i64,)> = sqlx::query_as(
        "SELECT m.id FROM question_messages m WHERE m.question_id = ? AND m.sender_id = ? ORDER BY m.id DESC LIMIT 1")
        .bind(qid).bind(row.9).fetch_optional(pool).await.map_err(dberr)?;
    sqlx::query("UPDATE questions SET status = 'PUBLISHED', published_at = UTC_TIMESTAMP(), approved_by = ?, published_message_id = ? WHERE id = ?")
        .bind(approver).bind(last.map(|l| l.0)).bind(qid)
        .execute(pool).await.map_err(dberr)?;
    transition(pool, qid, Some("PUBLISH_REQUESTED"), "PUBLISHED", Some(approver), Some("approved utk knowledge base")).await?;
    // notif penanya + ustadz
    for uid in [row.1, row.9.unwrap_or(0)] {
        if uid == 0 { continue; }
        sqlx::query("INSERT INTO user_notifications (user_id, template_code, title, body, data, channel) \
                     VALUES (?, 'QUESTION_PUBLISHED', 'Jawaban dipublikasikan', 'Jawaban tanya ustadz kini tampil di arsip publik', CAST(? AS JSON), 'IN_APP')")
            .bind(uid).bind(format!("{{\"deeplink\":\"question:{qid}\"}}"))
            .execute(pool).await.map_err(dberr)?;
    }
    Ok(())
}

pub async fn reject_publish(pool: &MySqlPool, approver: i64, qid: i64, reason: Option<String>) -> Result<(), AppError> {
    let row = fetch_q(pool, qid).await?;
    if row.8 != "PUBLISH_REQUESTED" {
        return Err(AppError::Conflict(format!("harus PUBLISH_REQUESTED (sekarang {})", row.8)));
    }
    sqlx::query("UPDATE questions SET status = 'REJECTED', updated_at = UTC_TIMESTAMP() WHERE id = ?")
        .bind(qid).execute(pool).await.map_err(dberr)?;
    transition(pool, qid, Some("PUBLISH_REQUESTED"), "REJECTED", Some(approver), reason.as_deref()).await?;
    sqlx::query("INSERT INTO user_notifications (user_id, template_code, title, body, data, channel) \
                 VALUES (?, 'QUESTION_PUBLISH_REJECTED', 'Publikasi ditolak', ?, CAST(? AS JSON), 'IN_APP')")
        .bind(row.1).bind(reason.clone().unwrap_or_else(|| "Jawaban tidak dipublikasikan".into()))
        .bind(format!("{{\"deeplink\":\"question:{qid}\"}}"))
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}

pub async fn detail(pool: &MySqlPool, storage: Option<&Storage>, viewer: i64, can_moderate: bool, can_admin: bool, qid: i64)
    -> Result<QuestionThread, AppError> {
    let row = fetch_q(pool, qid).await?;
    let allowed = access(pool, viewer, can_moderate, can_admin, &row);
    if !allowed {
        return Err(AppError::NotFound("pertanyaan tidak ditemukan".into()));
    }
    // PUBLISHED thread bisa dilihat siapa saja (arsip) — tapi pesan hanya utk peserta? Kontrak: arsip menampilkan Q&A.
    let is_owner = row.1 == viewer;
    thread(pool, storage, qid, can_moderate, is_owner).await
}
