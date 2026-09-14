# MQ — ERD Ringkas (Task 1.3)

> Diagram lengkap (interaktif): buka [dbdiagram.io/d](https://dbdiagram.io/d) → paste `docs/erd.dbml` (generate otomatis via `python scripts/gen_erd.py` dari `mq_dev` aktual — 47 tabel, 69 relasi FK).

## Peta domain (7 kelompok)

### 1. Identitas & RBAC (0003, 0005)
`users` (BIGINT id, phone/email partial-unique, account_type, status **PENDING_VERIFICATION/ACTIVE/SUSPENDED/DEACTIVATED/DELETED**) → `user_profiles`, `ustadz_profiles` (1:1; ustadz: `is_accepting_questions`, `max_active_questions`, `verified_at` = gate review/jawab), `user_devices` (FCM token unik), `user_sessions` (refresh_token_hash unik; ip VARCHAR(45)).
RBAC: `roles` ↔ `permissions` (N:M `role_permissions`) ← `user_roles` (N:M ke users, assigned_by self-FK).

### 2. Quran reference (0006) — IMPORT-ONLY, tanpa CRUD
`quran_surahs` (PK natural 1–114) → `quran_ayahs` (UNIQUE surah+ayah; text_uthmani/imlaei, juz 1–30, page 1–604, sajda) → `quran_translations` (UNIQUE ayah+translator; KEMENAG), `quran_audio_files` (UNIQUE ayah+reciter; URL CDN), **`quran_words`** (UNIQUE ayah+position; WBW 77.429 kata: uthmani+transliteration+EN; `text_id` ID-menyusul-Phase-2). `quran_juzs` (PK natural 1–30, rentang start/end). Tajwid: `tajwid_rules` (+ `tajwid_ayah_annotations` **DORMANT** — kosong, Phase 2).

### 3. Learning & hafalan (0007, 0008)
`user_reading_progress` (1:1 users — last-read hot path Home), `bookmarks` (UNIQUE user+ayah), `learning_materials` (slug unik, tajwid_rule_id FK, status DRAFT/PUBLISHED/ARCHIVED).
`memorization_submissions` (status **PENDING → IN_REVIEW → PASSED/REVISION/REJECTED**, ustadz_id diisi saat claim; queue index status+ustadz+submitted_at) → `memorization_reviews` (1:1, verdict + reply voice media) ; `memorization_progress` rollup (PK user+surah, ditulis service saat PASSED). *(kolom `client_key` idempotency menyusul via 0015 Bagian III)*

### 4. Khatmil (0009)
`khatmil_campaigns` (mode PARALLEL/SEQUENTIAL, `min_minutes_per_juz`, require_manual_verification) → `khatmil_participants` (UNIQUE campaign+user) → `khatmil_juz_assignments` (**UNIQUE `(campaign_id, juz, active_marker)`** — active_marker = generated kolom dari status ⇒ SATU pemegang aktif per juz; terverifikasi ERROR 1062) → `khatmil_progress` (1:1 assignment; verification PENDING/SELF_REPORTED/AUTO_VERIFIED/SYSTEM_VERIFIED/VERIFIED/REJECTED) + `khatmil_progress_events` (append-only audit) + `khatmil_completions` (UNIQUE campaign+cycle).

### 5. Tanya Ustadz (0010) — moderation state machine
`question_categories` (seed 6) ← `ustadz_specializations` (ustadz N:M kategori). `questions` (status **QUEUED → ASSIGNED → ANSWERED → PUBLISH_REQUESTED → PUBLISHED | REJECTED** (+CLOSED); anonim flag; FULLTEXT title+body; approved_by/published_at) → `question_messages` (TEXT/VOICE/IMAGE/FILE; media_id), `question_assignments` (rotasi least-load: index status+ustadz), `question_status_history` (audit transisi + SLA timestamps).

### 6. CMS & notifikasi (0011, 0012)
`article_categories`/`articles`/`banners`/`announcements`/`faqs` (status publish + periode aktif). `notification_templates` (kode + title/body template) → `user_notifications` (per user, channel IN_APP/PUSH/EMAIL/WA, data JSON deeplink, read_at).

### 7. Cross-cutting (0013)
`audit_logs` (actor, entity_type/id, old/new JSON, ip), `activity_events` (**sumber Learning Journey/Home**; index user+occurred & type+occurred), `settings` (key-value JSON), `scheduled_jobs` (status PENDING/RUNNING/DONE/FAILED/DEAD, run_at, **dedupe_key UNIQUE**, locked_at stale-recovery; index status+run_at utk poll SKIP LOCKED) — worker eksekusi RUNNING di luar lock (v11).

`media` (0004) direferensikan lintas domain (owner users; dipakai setoran/review voice, attachment tanya, foto profil, banner/artikel) — satu registry S3 (SeaweedFS) dengan status UPLOADING/READY/FAILED.

## Catatan desain yang terlihat di ERD
- Semua ID BIGINT AUTO_INCREMENT SIGNED; enum = VARCHAR+CHECK (lihat 0002 registry).
- `ON DELETE CASCADE` untuk data milik-user (progress/bookmark/sessions); FK lintas domain tanpa cascade (histori aman, PDP delete-account = anonymize + status DELETED — bukan hard delete).
- TIMESTAMP = UTC (pool `time_zone='+00:00'`), `updated_at` auto ON UPDATE.
