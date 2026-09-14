-- =============================================================
-- MQ Digital Platform — 0002 Enum types
-- Semua enum didefinisikan di satu file agar urutan migration
-- tidak saling menunggu. Menambah nilai enum = ALTER TYPE ... ADD VALUE.
-- =============================================================

-- Identitas
CREATE TYPE account_type  AS ENUM ('SANTRI', 'UMUM', 'PENGURUS', 'ORANG_TUA', 'DONATUR'); -- extensible
CREATE TYPE user_status   AS ENUM ('PENDING_VERIFICATION', 'ACTIVE', 'SUSPENDED', 'DEACTIVATED');
CREATE TYPE gender        AS ENUM ('MALE', 'FEMALE');
CREATE TYPE device_platform AS ENUM ('ANDROID', 'IOS', 'WEB');

-- Media / object storage (R2 / MinIO)
CREATE TYPE media_kind   AS ENUM ('AUDIO', 'IMAGE', 'VIDEO', 'DOCUMENT');
CREATE TYPE media_status AS ENUM ('UPLOADING', 'READY', 'FAILED');

-- Hafalan / setoran
CREATE TYPE submission_status AS ENUM ('PENDING', 'IN_REVIEW', 'PASSED', 'REVISION', 'REJECTED');
CREATE TYPE review_verdict    AS ENUM ('PASSED', 'REVISION', 'REJECTED');

-- Khotmil Qur'an
CREATE TYPE campaign_mode       AS ENUM ('PARALLEL', 'SEQUENTIAL');   -- PARALLEL = tiap orang baca juznya sendiri; SEQUENTIAL = giliran
CREATE TYPE campaign_status     AS ENUM ('DRAFT', 'SCHEDULED', 'ACTIVE', 'COMPLETED', 'CANCELLED');
CREATE TYPE juz_status          AS ENUM ('ASSIGNED', 'IN_PROGRESS', 'COMPLETED', 'EXPIRED', 'REASSIGNED');
CREATE TYPE verification_status AS ENUM ('PENDING', 'AUTO_VERIFIED', 'VERIFIED', 'REJECTED');

-- Tanya Ustadz
CREATE TYPE question_status           AS ENUM ('PENDING_MODERATION', 'QUEUED', 'ASSIGNED', 'ANSWERED', 'CLOSED', 'REJECTED', 'PUBLISHED');
CREATE TYPE message_type              AS ENUM ('TEXT', 'VOICE', 'IMAGE', 'FILE');
CREATE TYPE ustadz_assignment_status  AS ENUM ('ASSIGNED', 'ACCEPTED', 'DECLINED', 'REASSIGNED');

-- Notifikasi
CREATE TYPE notification_channel AS ENUM ('IN_APP', 'PUSH', 'EMAIL', 'WHATSAPP');

-- CMS
CREATE TYPE material_status     AS ENUM ('DRAFT', 'PUBLISHED', 'ARCHIVED');
CREATE TYPE article_status      AS ENUM ('DRAFT', 'PUBLISHED', 'ARCHIVED');
CREATE TYPE banner_position     AS ENUM ('HOME_TOP', 'HOME_MID', 'KHOTMIL_TOP');
CREATE TYPE announcement_level  AS ENUM ('INFO', 'WARNING', 'CRITICAL');

-- Background jobs (Postgres-backed queue, worker apalis / loop sqlx)
CREATE TYPE job_status AS ENUM ('PENDING', 'RUNNING', 'DONE', 'FAILED', 'DEAD');
