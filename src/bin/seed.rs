//! Seed idempotent: roles, permissions + mapping, kategori tanya, settings default,
//! admin pertama. `cargo run --bin seed` (DATABASE_URL menentukan target).
//! Sumber kebenaran mapping = docs/permissions.md.
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};

const ROLES: &[(&str, &str, &str)] = &[
    ("SUPER_ADMIN", "Super Admin", "Akses penuh termasuk roles & permissions"),
    ("ADMIN", "Admin", "Pengelolaan platform (users, konten, campaign)"),
    ("MODERATOR", "Moderator", "Moderasi tanya ustadz & publikasi jawaban"),
    ("USTADZ", "Ustadz", "Review setoran hafalan & menjawab pertanyaan"),
    ("SANTRI", "Santri", "Murojaah, khatmil, tanya ustadz"),
];

/// (module, code, description)
const PERMISSIONS: &[(&str, &str, &str)] = &[
    ("quran", "quran.read", "Baca data Al-Quran"),
    ("media", "media.upload", "Upload media (presign)"),
    ("notification", "notification.read.self", "Baca notifikasi sendiri"),
    ("account", "account.delete", "Hapus akun sendiri (PDP)"),
    ("memorization", "memorization.submit", "Kirim setoran hafalan"),
    ("memorization", "memorization.review", "Review setoran hafalan"),
    ("khatmil", "khatmil.join", "Join campaign & klaim juz"),
    ("khatmil", "khatmil.read", "Lihat campaign khatmil"),
    ("khatmil", "khatmil.manage", "Kelola campaign khatmil"),
    ("question", "question.create", "Buat pertanyaan"),
    ("question", "question.answer", "Menjawab pertanyaan (ustadz)"),
    ("question", "question.moderate", "Moderasi antrean pertanyaan"),
    ("question", "question.publish.moderate", "Approve/reject publish jawaban"),
    ("ustadz", "ustadz.profile.self", "Kelola profil & availability ustadz"),
    ("users", "users.read", "Lihat daftar pengguna"),
    ("users", "users.manage", "Kelola pengguna & status akun"),
    ("dashboard", "dashboard.view", "Lihat dashboard admin"),
    ("settings", "settings.manage", "Ubah settings"),
    ("audit", "audit.read", "Baca audit log"),
    ("rbac", "roles.manage", "Kelola roles & permissions"),
    ("visits", "visits.book", "Santri: pesan ustadz terdekat, chat transaksi, review ustadz"),
    ("visits", "ustadz.visits.manage", "Ustadz: settings kunjungan, tarif, confirm/decline/complete, review santri"),
    ("visits", "visits.admin", "Admin: monitoring kunjungan + pembayaran Xendit + force actions + moderasi review"),
];

/// role_code -> [permission codes] (sinkron docs/permissions.md)
const ROLE_PERMISSIONS: &[(&str, &[&str])] = &[
    ("SUPER_ADMIN", &[
        "quran.read", "media.upload", "notification.read.self",
        "account.delete", "khatmil.read", "khatmil.manage", "question.moderate",
        "question.publish.moderate", "users.read", "users.manage",
        "dashboard.view", "settings.manage", "audit.read", "roles.manage",
        "visits.admin",
    ]),
    ("ADMIN", &[
        "quran.read", "media.upload", "notification.read.self",
        "account.delete", "khatmil.read", "khatmil.manage", "question.moderate",
        "question.publish.moderate", "users.read", "users.manage",
        "dashboard.view", "settings.manage", "audit.read",
        "visits.admin",
    ]),
    ("MODERATOR", &[
        "quran.read", "notification.read.self", "account.delete",
        "khatmil.read", "question.moderate", "question.publish.moderate", "users.read",
        "visits.admin",   // fokus: moderasi review (hide/unhide) + monitoring
    ]),
    ("USTADZ", &[
        "quran.read", "media.upload", "notification.read.self",
        "account.delete", "khatmil.read", "memorization.review", "question.answer",
        "ustadz.profile.self", "ustadz.visits.manage",
    ]),
    ("SANTRI", &[
        "quran.read", "media.upload", "notification.read.self",
        "account.delete", "memorization.submit", "khatmil.join", "khatmil.read",
        "question.create", "visits.book",
    ]),
];

const CATEGORIES: &[(&str, &str)] = &[
    ("fiqh", "Fiqh"),
    ("ngaji", "Ngaji / Tahsin"),
    ("tajwid", "Tajwid"),
    ("akhlaq", "Akhlak"),
    ("keluarga", "Keluarga"),
    ("muamalah", "Muamalah"),
];

/// (key, value JSON, description)
const SETTINGS: &[(&str, &str, &str)] = &[
    ("registration_enabled", "true", "Pendaftaran santri terbuka"),
    ("registration_require_verification", "true", "Akun harus verifikasi email / aktivasi admin (keputusan #14)"),
    ("voice_note_max_mb", "10", "Batas ukuran file audio setoran (MB)"),
    ("voice_note_max_minutes", "5", "Batas durasi audio setoran (menit)"),
    ("question_moderation_required", "true", "Pertanyaan baru masuk antrean moderasi (opsional via settings)"),
    // --- Bagian V: Pesan Ustadz (visits) ---
    ("visit_enabled", "false", "Kill-switch modul Pesan Ustadz (default OFF sampai rilis)"),
    ("visit_min_schedule_hours", "2", "Jadwal kunjungan minimal H+N jam dari sekarang"),
    ("visit_max_schedule_days", "14", "Jadwal kunjungan maksimal H+N hari ke depan"),
    ("visit_confirm_timeout_hours", "3", "Batas ustadz konfirmasi setelah dibayar; lewat = auto-decline + refund"),
    ("visit_cancel_free_hours", "2", "Cancel santri >= N jam sebelum jadwal = full refund; < N = tanpa refund"),
    ("visit_radius_km", "5", "Radius GLOBAL pencarian ustadz terdekat (diatur admin mq-admin)"),
    ("visit_review_window_days", "7", "Window double-blind rating: reveal otomatis setelah N hari sejak COMPLETED"),
    ("visit_location_fresh_hours", "6", "Lokasi ustadz lebih tua dari N jam tidak muncul di nearby"),
    ("visit_invoice_duration_sec", "7200", "Masa berlaku invoice Xendit (detik)"),
    ("visit_minor_booking_policy", "\"guardian_required\"", "Flag kebijakan anak (live review; gate wali = backlog, belum berefek)"),
    // --- Khatmil: pengingat juz mangkrak (plan admin rev 3.3 F1.8) ---
    ("khatmil_reminder_enabled", "false", "Kill-switch pengingat otomatis juz mangkrak (default OFF — admin aktifkan dari Pengaturan)"),
    ("khatmil_reminder_stale_days", "3", "Juz dianggap mangkrak bila terakhir lapor > N hari (0 = langsung)"),
];

/// Bagian V: master jenis layanan kunjungan (id TINYINT tetap — jangan reorder)
const VISIT_SERVICE_TYPES: &[(i64, &str, &str, &str)] = &[
    (1, "tahsin_privat", "Tahsin Privat", "Perbaikan bacaan Al-Qur'an satu-satu di rumah santri"),
    (2, "murajaah", "Murajaah / Setoran Hafalan", "Muroja'ah hafalan bersama ustadz di lokasi"),
    (3, "tahlil_yasinan", "Tahlil & Yasinan", "Menghadirkan ustadz untuk tahlil/yasinan keluarga"),
    (4, "konsultasi", "Konsultasi", "Konsultasi agama/keluarga tatap muka"),
];

/// Bagian V: notif templates (placeholder {{...}}; deeplink di data JSON utk visit:{id})
const VISIT_TEMPLATES: &[(&str, &str, &str)] = &[
    ("VISIT_PAID_WAITING", "Permintaan kunjungan baru", "Santri memesan layanan {{service}} untuk {{schedule}}. Segera konfirmasi atau tolak (batas {{timeout}} jam)."),
    ("VISIT_CONFIRMED", "Kunjungan dikonfirmasi", "Ustadz {{ustadz}} telah MENGONFIRMASI kunjungan {{service}} {{schedule}}. Kontak & chat kini terbuka."),
    ("VISIT_DECLINED_REFUNDED", "Permintaan ditolak ustadz", "Ustadz menolak kunjungan {{schedule}}. Dana PENUH dikembalikan ke metode pembayaran Anda."),
    ("VISIT_CANCELED", "Kunjungan dibatalkan", "Kunjungan {{schedule}} dibatalkan ({{by}}). {{refund_note}}"),
    ("VISIT_REMINDER", "Pengingat kunjungan", "Kunjungan {{service}} bersama {{ustadz}} kurang {{hours}} jam lagi. Lokasi: {{label}}."),
    ("VISIT_COMPLETED_PLEASE_REVIEW", "Kunjungan selesai", "Alhamdulillah, kunjungan {{schedule}} selesai. Beri rating & catatan untuk ustadz Anda."),
    ("VISIT_REVIEW_USTADZ_PENDING", "Nilai santri Anda", "Kunjungan {{schedule}} sudah selesai. Beri rating & catatan untuk santri Anda (privat, double-blind)."),
    ("VISIT_REFUND_PENDING_MANUAL", "Refund manual diperlukan", "Payment {{external_id}} butuh refund manual dari dashboard Xendit ({{reason}}). Tandai setelah selesai."),
    // Khatmil: pengingat juz mangkrak (plan admin rev 3.3 F1.8)
    ("KHATMIL_JUZ_STALE", "Pengingat Juz", "Juz {{juz}} di campaign {{campaign}} belum selesai — yuk lanjutkan bacaannya"),
];

async fn q(pool: &sqlx::MySqlPool, sql: &str, binds: &[&str]) {
    let mut stmt = sqlx::query(sql);
    for b in binds {
        stmt = stmt.bind(b);
    }
    stmt.execute(pool).await.unwrap_or_else(|e| panic!("{sql:.60}...: {e}"));
}

async fn one(pool: &sqlx::MySqlPool, sql: &str, bind: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(sql)
        .bind(bind)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("{sql:.60}: {e}"))
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL wajib");
    let opts: MySqlConnectOptions = url.parse().expect("DATABASE_URL tidak valid");
    let opts = opts.timezone(Some("+00:00".to_string()));
    let pool = MySqlPoolOptions::new().max_connections(2).connect_with(opts).await.expect("connect DB");

    // 1. Roles
    for (code, name, desc) in ROLES {
        q(&pool, "INSERT INTO roles (code, name, description) VALUES (?, ?, ?) AS new \
                  ON DUPLICATE KEY UPDATE name = new.name, description = new.description",
          &[code, name, desc]).await;
    }
    println!("roles: {} upsert", ROLES.len());

    // 2. Permissions
    for (module, code, desc) in PERMISSIONS {
        q(&pool, "INSERT INTO permissions (module, code, description) VALUES (?, ?, ?) AS new \
                  ON DUPLICATE KEY UPDATE module = new.module, description = new.description",
          &[module, code, desc]).await;
    }
    println!("permissions: {} upsert", PERMISSIONS.len());

    // 3. Role <-> permission mapping (idempotent; matrix = sumber kebenaran)
    let mut n_map = 0usize;
    for (role, perms) in ROLE_PERMISSIONS {
        let role_id = one(&pool, "SELECT id FROM roles WHERE code = ?", role).await;
        for p in *perms {
            let pid = one(&pool, "SELECT id FROM permissions WHERE code = ?", p).await;
            q(&pool, "INSERT INTO role_permissions (role_id, permission_id) VALUES (?, ?) AS new \
                      ON DUPLICATE KEY UPDATE role_id = new.role_id",
              &[&role_id.to_string(), &pid.to_string()]).await;
            n_map += 1;
        }
    }
    println!("role_permissions: {n_map} mapping di-apply");

    // 4. Kategori tanya
    for (i, (slug, name)) in CATEGORIES.iter().enumerate() {
        q(&pool, "INSERT INTO question_categories (slug, name, sort_order) VALUES (?, ?, ?) AS new \
                  ON DUPLICATE KEY UPDATE name = new.name, sort_order = new.sort_order",
          &[slug, name, &(i + 1).to_string()]).await;
    }
    println!("kategori: {} upsert", CATEGORIES.len());

    // 4b. Bagian V: jenis layanan kunjungan (id fix, idempotent)
    for (id, code, name, desc) in VISIT_SERVICE_TYPES {
        q(&pool, "INSERT INTO visit_service_types (id, code, name, description, sort_order) \
                  VALUES (?, ?, ?, ?, ?) AS new \
                  ON DUPLICATE KEY UPDATE code = new.code, name = new.name, description = new.description, sort_order = new.sort_order",
          &[&id.to_string(), code, name, desc, &id.to_string()]).await;
    }
    println!("visit_service_types: {} upsert", VISIT_SERVICE_TYPES.len());

    // 4c. Bagian V: notif templates kunjungan
    for (code, title, body) in VISIT_TEMPLATES {
        q(&pool, "INSERT INTO notification_templates (code, title_template, body_template) VALUES (?, ?, ?) AS new \
                  ON DUPLICATE KEY UPDATE title_template = new.title_template, body_template = new.body_template",
          &[code, title, body]).await;
    }
    println!("visit templates: {} upsert", VISIT_TEMPLATES.len());

    // 5. Settings default
    for (key, value, desc) in SETTINGS {
        q(&pool, "INSERT INTO settings (`key`, value, description) VALUES (?, CAST(? AS JSON), ?) AS new \
                  ON DUPLICATE KEY UPDATE description = new.description",
          &[key, value, desc]).await;
    }
    println!("settings: {} upsert", SETTINGS.len());

    // 6. Admin pertama (idempotent: hanya jika belum ada user SUPER_ADMIN)
    let super_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE r.code = 'SUPER_ADMIN'")
        .fetch_one(&pool).await.unwrap();
    if super_count == 0 {
        let email = std::env::var("MQ_SEED_ADMIN_EMAIL").unwrap_or_else(|_| "admin@mq.local".into());
        let password = std::env::var("MQ_SEED_ADMIN_PASSWORD").unwrap_or_else(|_| {
            use std::fmt::Write;
            let mut p = String::new();
            for b in uuid::Uuid::new_v4().as_bytes().iter().take(6) { let _ = write!(p, "{b:02x}"); }
            p
        });
        let salt = argon2::password_hash::SaltString::encode_b64(uuid::Uuid::new_v4().as_bytes())
            .expect("salt");
        use argon2::PasswordHasher;
        let hash = argon2::Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .expect("argon2 hash")
            .to_string();

        let result = sqlx::query(
            "INSERT INTO users (email, password_hash, account_type, status, email_verified_at) \
             VALUES (?, ?, 'PENGURUS', 'ACTIVE', UTC_TIMESTAMP())")
            .bind(&email).bind(&hash)
            .execute(&pool).await.expect("insert admin");
        let uid = result.last_insert_id();
        q(&pool, "INSERT INTO user_roles (user_id, role_id) \
                  SELECT ?, id FROM roles WHERE code = 'SUPER_ADMIN'",
          &[&uid.to_string()]).await;
        println!("ADMIN PERTAMA -> email: {email} | password: {password}");
        println!("(simpan sekarang — password tidak dicatat di mana pun)");
    } else {
        println!("admin: sudah ada ({super_count} super admin) — skip");
    }

    // Ringkasan
    for (label, sqlq) in [
        ("roles", "SELECT COUNT(*) FROM roles"),
        ("permissions", "SELECT COUNT(*) FROM permissions"),
        ("role_permissions", "SELECT COUNT(*) FROM role_permissions"),
        ("kategori", "SELECT COUNT(*) FROM question_categories"),
        ("settings", "SELECT COUNT(*) FROM settings"),
        ("visit_types", "SELECT COUNT(*) FROM visit_service_types"),
        ("notif_templates", "SELECT COUNT(*) FROM notification_templates"),
    ] {
        let n: i64 = sqlx::query_scalar(sqlq).fetch_one(&pool).await.unwrap();
        println!("{label}: {n}");
    }
    println!("seed OK");
}
