# mq-backend

Backend API MQ Mujayarotul Faqih — Pesantren Digital Platform.

- Rust (Axum + Tokio + SQLx) → REST `/api/v1`
- MySQL 8 (instance Laragon, port 3306; DB `mq_dev`/`mq_test`, user `mq_app`)
- MinIO (S3) via NSSM service — object storage media
- Queue: `scheduled_jobs` (Postgres-style → MySQL, `FOR UPDATE SKIP LOCKED`)
- Cache & rate-limit: in-process (moka + governor), adapter `CacheStore`

**Plan (sumber kebenaran):** `~/.hermes/plans/mq-platform-plan-gabungan-rev11.md` — Bagian I (master).

## Struktur
```
migrations/   # skema (0001-0013 = hasil konversi PG→MySQL Task 1.0; 0014-0016 mobile)
src/          # modules per domain (Bagian I Phase 3)
```

## Dev
```
cargo build --target x86_64-pc-windows-msvc   # PC ARM64 → WAJIB target x86_64
cargo run --target x86_64-pc-windows-msvc     # /healthz
sqlx migrate run                              # setelah Task 1.0
```
