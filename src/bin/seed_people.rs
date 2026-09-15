//! Seed People — 70 santri + 30 ustadz bernama asli Indonesia (plan 2026-09-15_mq-seed-people.md).
//! Idempotent (backfill profil/role utk user yang sudah ada). Tidak menyentuh akun lain.
//! Run: DATABASE_URL=... cargo run --release --bin seed_people

use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions};

struct Person {
    first: &'static str,
    last: &'static str,
    gender: &'static str, // M / F  -> disimpan MALE/FEMALE
    role: &'static str,   // SANTRI / USTADZ
    kota: &'static str,
    prov: &'static str,
    bio: &'static str,
    title: &'static str,
    spec: &'static [i64],
}

const KOTA: &[(&str, &str)] = &[
    ("Yogyakarta", "DI Yogyakarta"), ("Solo", "Jawa Tengah"), ("Semarang", "Jawa Tengah"),
    ("Surabaya", "Jawa Timur"), ("Malang", "Jawa Timur"), ("Gresik", "Jawa Timur"),
    ("Jakarta", "DKI Jakarta"), ("Bandung", "Jawa Barat"), ("Bekasi", "Jawa Barat"),
    ("Tasikmalaya", "Jawa Barat"),
];

const BIOS_S: &[&str] = &[
    "Santri aktif program tahfidz, semangat khatmil bersama.",
    "Pengajar TPQ di sela kuliah, gemar belajar tajwid.",
    "Santri baru, ingin konsisten menuntaskan khatmil pertama.",
    "Aktif di majelis tadabbur pekanan dan kajian akhlak.",
    "Pekerja swasta, menuntaskan khataman tiap bulan lewat khatmil.",
    "Mahasiswa, murajaah setiap subuh di mushaf kampus.",
    "Ibu rumah tangga, khatmil jadi pengingat wirid harian.",
    "Alumni pesantren, menjaga setoran hafalan pekanan.",
];

const BIOS_U: &[&str] = &[
    "Pengasuh TPQ dan pembina kajian keluarga.",
    "Asatidz tahfidz, fokus pembinaan mutaba'ah hafalan.",
    "Dosen pengajar bahasa Arab dan fiqh ibadah.",
    "Imam dan mubaligh desa, aktif majelis taklim.",
    "Mentor tahsin dewasa, pendekatan talaqqi.",
    "Pengurus lembaga pendidikan islam, pembina santri mukim.",
];

include!("seed_people_data.rs");

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL wajib");
    eprintln!("DB: {}", url.split('@').nth(1).unwrap_or("?"));
    let opts: MySqlConnectOptions = url.parse().expect("DATABASE_URL tidak valid");
    let opts = opts.timezone(Some("+00:00".to_string()));
    let pool = MySqlPoolOptions::new().max_connections(2).connect_with(opts).await.expect("connect DB");

    let salt = argon2::password_hash::SaltString::encode_b64(uuid::Uuid::new_v4().as_bytes()).expect("salt");
    use argon2::PasswordHasher;
    let hash = argon2::Argon2::default()
        .hash_password(b"mqdemo123", &salt)
        .expect("argon2 hash")
        .to_string();

    let mut n_santri = 0u32;
    let mut n_ustadz = 0u32;
    let mut n_profile = 0u32;
    let mut n_role = 0u32;
    let mut n_ustadz_profile = 0u32;

    for (i, p) in PEOPLE.iter().enumerate() {
        let email = format!(
            "{}.{}@mq-demo.id",
            p.first.to_lowercase().replace(' ', "."),
            p.last.to_lowercase().replace(' ', ".")
        );
        let gender = if p.gender == "M" { "MALE" } else { "FEMALE" };
        let account_type = if p.role == "USTADZ" { "PENGURUS" } else { "SANTRI" };

        // 1. user (idempotent by email — backfill profil bila sudah ada)
        let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
            .bind(&email)
            .fetch_optional(&pool)
            .await
            .unwrap_or_else(|e| panic!("cek email {email}: {e}"));
        let uid: i64 = match existing {
            Some(uid) => uid,
            None => {
                let r = sqlx::query(
                    "INSERT INTO users (email, password_hash, account_type, status, email_verified_at) \
                     VALUES (?, ?, ?, 'ACTIVE', UTC_TIMESTAMP())")
                    .bind(&email).bind(&hash).bind(account_type)
                    .execute(&pool).await
                    .unwrap_or_else(|e| panic!("insert user {email}: {e}"));
                r.last_insert_id() as i64
            }
        };

        // 2. profil (backfill bila belum ada)
        let has_profile: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_profiles WHERE user_id = ?")
            .bind(uid).fetch_one(&pool).await.unwrap_or(1);
        if has_profile == 0 {
            let (bd_y, bd_m) = if p.role == "SANTRI" { (1990, 2008) } else { (1980, 1998) };
            let byear = bd_y + ((uid as u32) % (bd_m - bd_y));
            let bmonth = 1 + (uid as u32 * 7) % 12;
            let bday = 1 + (uid as u32 * 13) % 28;
            let kota = KOTA[(uid as usize) % KOTA.len()];
            let r = sqlx::query(
                "INSERT INTO user_profiles (user_id, full_name, gender, birth_date, address_text, city, province, bio) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(uid).bind(format!("{} {}", p.first, p.last)).bind(gender)
                .bind(format!("{byear:04}-{bmonth:02}-{bday:02}"))
                .bind(format!("Perum {}", kota.0)).bind(kota.0).bind(kota.1).bind(p.bio)
                .execute(&pool).await
                .unwrap_or_else(|e| panic!("insert profile {email}: {e}"));
            n_profile += r.rows_affected() as u32;
        } else {
            n_profile += 0;
        }

        // 3. role (backfill)
        let has_role: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_roles ur JOIN roles r ON r.id = ur.role_id \
             WHERE ur.user_id = ? AND r.code = ?")
            .bind(uid).bind(p.role).fetch_one(&pool).await.unwrap_or(0);
        if has_role == 0 {
            let role_id: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE code = ?")
                .bind(p.role).fetch_one(&pool).await
                .unwrap_or_else(|e| panic!("role {}: {e}", p.role));
            sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
                .bind(uid).bind(role_id)
                .execute(&pool).await.expect("insert user_roles");
            n_role += 1;
        }

        // 4. ustadz profile + spesialisasi
        if p.role == "USTADZ" {
            let has_up: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ustadz_profiles WHERE user_id = ?")
                .bind(uid).fetch_one(&pool).await.unwrap_or(0);
            if has_up == 0 {
                let code = format!("UST-{:04}", uid);
                sqlx::query(
                    "INSERT INTO ustadz_profiles (user_id, code, title, bio, is_accepting_questions, max_active_questions, verified_at) \
                     VALUES (?, ?, ?, ?, 1, 5, UTC_TIMESTAMP())")
                    .bind(uid).bind(&code).bind(p.title).bind(p.bio)
                    .execute(&pool).await
                    .unwrap_or_else(|e| panic!("insert ustadz_profiles {email}: {e}"));
                n_ustadz_profile += 1;
            }
            for cat in p.spec {
                let has_spec: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM ustadz_specializations WHERE ustadz_id = ? AND category_id = ?")
                    .bind(uid).bind(cat).fetch_one(&pool).await.unwrap_or(0);
                if has_spec == 0 {
                    sqlx::query("INSERT INTO ustadz_specializations (ustadz_id, category_id) VALUES (?, ?)")
                        .bind(uid).bind(cat)
                        .execute(&pool).await.expect("insert spesialisasi");
                }
            }
            n_ustadz += 1;
        } else {
            n_santri += 1;
        }
        if i == 0 {
            eprintln!("DEBUG person#1 uid={uid} profile=ok role=ok");
        }
    }

    let cu: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&pool).await.unwrap_or(0);
    let cp: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_profiles").fetch_one(&pool).await.unwrap_or(0);
    let cr: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_roles").fetch_one(&pool).await.unwrap_or(0);
    println!("verify-in-app: users={cu} profiles={cp} roles={cr}");
    println!("seed_people OK: +{n_santri} santri, +{n_ustadz} ustadz | profil baru {n_profile}, role baru {n_role}, ustadz-profil baru {n_ustadz_profile}");
}
