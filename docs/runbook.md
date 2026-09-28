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

## Media via tunnel (22Sep)
- SeaweedFS S3 (port 9000) di-expose via tunnel **mq-media.jagodigital.online** (ingress + `originRequest.httpHostHeader: mq-media.jagodigital.online` — WAJIB; tanpa ini Host di-rewrite jadi localhost:9000 dan signature S3 403).
- `.env` S3_ENDPOINT = `https://mq-media.jagodigital.online` — presign URL (foto profil + cover khatmil) HARUS host publik ini agar bisa di-fetch device/browser.
- Cloudflare bot check menolak UA non-browser (python 1010); UA Dart/browser normal lolos — smoke harus set UA.
- Backup config cloudflared: config.yml.bak-media.

## PROD — server synergy .116 (2 Okt 2026)
Domain: **api.mq.synergyinfinity.id** (BE :8321) · **admin.mq.synergyinfinity.id** (Next standalone pm2 `mq-admin` :3210) · **media.mq.synergyinfinity.id** (SeaweedFS S3 :9000, path-style, `force_path_style`). Cert LE SAN-3-domain via acme.sh (webroot /www/wwwroot/mq-challenge; auto-renew + reloadcmd nginx reload). DNS wildcard *.mq → 103.167.113.116 (PowerDNS di server).

- **Build**: on-server — clone ke /root/mq-build/mq-backend → `cargo build --release -j 2` (rustup ada; `-j 2` demi RAM multi-tenant). Binary → /opt/mq/.
- **Services**: systemd `mq-backend.service`, `mq-worker.service` (EnvironmentFile /etc/mq/mq-backend.env, chmod 600), `seaweed.service` (weed server 9333/8080/8888/9000; **JANGAN pakai -volume.max=0** di 4.47-linux — 0 = benar2 nol volume, upload 500 "No writable volumes"). Data storage: /www/wwwroot/mq-media-data.
- **Admin**: build di server (npm ci + `npm run build`, Next standalone) → deploy /www/wwwroot/mq-admin → pm2 `mq-admin` (env inline: PORT=3210 HOSTNAME=127.0.0.1 NODE_ENV=production **API_BASE_URL=https://api.mq.synergyinfinity.id**; .env.production di-ignore git). GOTCHA: daemon pm2 v6 TIDAK meneruskan env ecosystem file → start ulang dengan env inline + `--cwd`.
- **DB**: MariaDB 10.11 `mq_prod` (user mq_app@127.0.0.1). Migrasi via bin `dbmigrate` dari /root/mq-deploy (butuh folder ./migrations di CWD). Kompatibilitas MySQL8→MariaDB sudah dipatch di file (DROP CHECK→DROP CONSTRAINT, CAST AS JSON→literal, 0006b no-op, 0022 patch v1→v2 fresh-install, seed VALUES(col)+tanpa visit_service_types).
- **Update BE/worker**: git pull di /root/mq-build → cargo build → cp ke /opt/mq → `systemctl restart mq-backend mq-worker` → curl healthz.
- **Update admin**: git pull /root/mq-build/mq-admin → npm ci + npm run build → rsync .next/standalone + .next/static + public ke /www/wwwroot/mq-admin → `pm2 restart mq-admin`.
- **CORS**: EXTRA_CORS_ORIGINS di /etc/mq/mq-backend.env (tanpa recompile).
- **Known**: HTTP:80 IP-publik ditangani edge hosting — host tak terdaftar dibalas 444 (semua vhost manual baru kena; 443 normal). Akses selalu https.
