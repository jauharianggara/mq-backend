# MQ Backend — Runbook (Phase 5.8)

> Operasional harian MQ Digital Platform backend. Semua command dijalankan dari repo root `C:\Users\jauha\ZCodeProject\mq-backend`.

## Stack

| Komponen | Port | Auto-start | Lokasi |
|---|---|---|---|
| MySQL 8.0.30 (Laragon) | 3306 | ✅ | `C:\laragon\bin\mysql\mysql-8.0.30-winx64` |
| SeaweedFS (S3) | 9000/9001 | ✅ NSSM `MQ-SeaweedFS` | `C:\mq\seaweedfs\weed.exe` (data: `C:\mq-data\seaweedfs`) |
| BE mq-backend | 8290 | via `start-mq-stack.bat` (Startup) | `target\x86_64-pc-windows-msvc\release\mq-backend.exe` |
| Worker | — | via `start-mq-stack.bat` (Startup) | `target\x86_64-pc-windows-msvc\release\worker.exe` |
| Admin mq-admin | 3210 | via `start-mq-stack.bat` | `mq-admin\` (npm run start) |
| Cloudflared | outbound | manual / script | `C:\Program Files (x86)\cloudflared\cloudflared.exe` |

## Domain Publik
- **API**: `https://mq-api.jagodigital.online` → localhost:8290
- **Admin**: `https://mq-admin.jagodigital.online` → localhost:3210

## Start/Stop Stack

```powershell
# Start semua (idempotent — cek yang jalan, nyalakan yang mati)
powershell "& C:\Users\jauha\ZCodeProject\mq-backend\scripts\start-mq-stack.bat"

# Stop BE + worker
taskkill /F /IM mq-backend.exe
taskkill /F /IM worker.exe
```

## Cek Kesehatan (checklist harian)

### 1. Backend hidup
```powershell
curl https://mq-api.jagodigital.online/healthz
# harus: {"data":{"status":"ok"},"meta":{}}
```

### 2. Worker hidup (tidak menumpuk)
```sql
-- cek backlog
SELECT status, COUNT(*) FROM scheduled_jobs GROUP BY status;
-- PENDING harus < 20, RUNNING harus 0-1, DEAD harus 0
-- kalau DEAD > 0: cek last_error
SELECT id, job_type, last_error, attempts FROM scheduled_jobs WHERE status = 'DEAD';
```

### 3. Disk space
```powershell
Get-PSDrive C | Select-Object Used,Free
# alert kalau free < 20GB (voice note tumbuh terus)
```

### 4. SeaweedFS
```powershell
sc query MQ-SeaweedFS   # harus RUNNING
curl http://127.0.0.1:9000  # harus 403 (anon)
```

### 5. Admin panel
```powershell
curl https://mq-admin.jagodigital.online/login  # harus 200
```

## Backup (otomatis Task Scheduler 02:00)

```powershell
# Lokasi: C:\mq-data\backups\mq\mq-YYYYMMDD.sql.gz (retensi 14 hari)
# Restore:
gunzip < mq-YYYYMMDD.sql.gz | mysql -u mq_app -p<PASS> mq_dev
```

Password DB: `~/.mq_dbpass.txt` (JANGAN commit)

## Troubleshooting

### Backend 502 (domain mati)
1. `tasklist | findstr mq-backend` — kalau tidak ada: jalankan start-mq-stack.bat
2. `curl http://127.0.0.1:8290/healthz` — kalau OK tapi domain 502 → cloudflared mati → restart
3. Cek log: `type run-mq.err.log` (error terakhir)

### Worker tidak jalan
1. `tasklist | findstr worker.exe`
2. Kalau tidak ada: `Start-Process ...worker.exe` (lihat start-mq-stack.bat)
3. Kalau DEAD jobs ada: cek `last_error`, kemungkinan butuh restart manual

### SeaweedFS mati
```powershell
C:\mq\nssm\nssm.exe restart MQ-SeaweedFS
```

### MySQL mati
1. `sc query MySQL` — kalau STOPPED: `net start MySQL`
2. Kalau service tidak ada: buka Laragon → Start All

### Ganti versi MySQL di Laragon
1. mysqldump penuh: `mysqldump -u mq_app -p... mq_dev > backup.sql`
2. Ganti versi di Laragon
3. Restore: `mysql -u mq_app -p... mq_dev < backup.sql`
⚠️ Selalu backup dulu!

## Reset Dev Data (HATI-HATI)
```sql
-- hapus semua data uji (JANGAN di produksi!)
DELETE FROM memorization_submissions;
DELETE FROM memorization_reviews;
DELETE FROM memorization_progress;
DELETE FROM khatmil_progress_events;
DELETE FROM khatmil_progress;
DELETE FROM khatmil_juz_assignments;
DELETE FROM khatmil_participants;
DELETE FROM khatmil_campaigns;
DELETE FROM question_status_history;
DELETE FROM question_assignments;
DELETE FROM question_messages;
DELETE FROM questions;
DELETE FROM activity_events;
DELETE FROM audit_logs;
DELETE FROM user_notifications;
DELETE FROM scheduled_jobs;
DELETE FROM auth_action_tokens;
DELETE FROM user_sessions;
DELETE FROM user_devices;
DELETE FROM user_reading_progress;
DELETE FROM bookmarks;
DELETE FROM media;
```

## Deployment Baru

```powershell
# 1. pull latest
cd C:\Users\jauha\ZCodeProject\mq-backend
git pull

# 2. migrate (idempotent)
cargo run --release --target x86_64-pc-windows-msvc --bin dbmigrate

# 3. rebuild + restart
cargo build --release --target x86_64-pc-windows-msvc
taskkill /F /IM mq-backend.exe; sleep 2
# start-mq-stack.bat akan restart semua
```
