//! Database migrator sederhana — `cargo run --bin dbmigrate`
//! (DATABASE_URL menentukan target; pool time_zone UTC + multi_statements).
//! Tracking di tabel `_mq_migration` (idempotent: file yang sudah apply di-skip).
//! Catatan: `sqlx::migrate!` macro E0433 di toolchain 1.98.1/x86_64 — runner
//! manual ini deterministik dan cukup untuk DDL murni (tanpa DELIMITER/procedure).
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL wajib (mis. mysql://mq_app:pass@127.0.0.1:3306/mq_dev)");

    let opts: MySqlConnectOptions = url.parse().expect("DATABASE_URL tidak valid");
    let opts = opts.timezone(Some("+00:00".to_string()));

    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .expect("gagal connect DB");

    let db: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&pool)
        .await
        .expect("SELECT DATABASE()");

    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _mq_migration (
            name VARCHAR(255) PRIMARY KEY,
            applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        ) ENGINE=InnoDB",
    )
    .execute(&pool)
    .await
    .expect("buat tabel tracking");

    let applied: Vec<String> =
        sqlx::query_scalar("SELECT name FROM _mq_migration")
            .fetch_all(&pool)
            .await
            .expect("baca tracking");

    let cwd = std::env::current_dir().expect("cwd");
    let dir = cwd.join("migrations");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("baca folder {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "sql"))
        .collect();
    files.sort();

    let total = files.len();
    for f in files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        if applied.iter().any(|a| a == &name) {
            println!("skip   {name}");
            continue;
        }
        let sql = std::fs::read_to_string(&f)
            .unwrap_or_else(|e| panic!("baca {}: {e}", f.display()));
        sqlx::raw_sql(&sql)
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("APPLY GAGAL {name}: {e}"));
        sqlx::query("INSERT INTO _mq_migration (name) VALUES (?)")
            .bind(&name)
            .execute(&pool)
            .await
            .expect("catat migration");
        println!("apply  {name}");
    }

    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE()",
    )
    .fetch_one(&pool)
    .await
    .expect("count tables");
    println!("migrate OK -> {db}: {n} tabel (dari {total} file)");
}
