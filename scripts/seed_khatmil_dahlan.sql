-- =============================================================
-- MQ — Seed contoh KHATMIL (campaign 30 "Khataman Santri MQ #1")
-- Santri Uji (santri.test@mq.local, user 2) = santri juz 26
-- Ustadz Ahmad Dahlan Munflih (user 415) = pembina kelompok
-- Idempotent: aman dijalankan berulang.
-- =============================================================
SET time_zone = '+00:00';

-- 1) Kelompok 1 campaign 30 dengan pembina langsung aktif
INSERT INTO khatmil_groups (campaign_id, group_no, ustadz_id, pending_ustadz_id)
SELECT 30, 1, 415, NULL
WHERE NOT EXISTS (SELECT 1 FROM khatmil_groups WHERE campaign_id = 30 AND group_no = 1);
SET @g := (SELECT id FROM khatmil_groups WHERE campaign_id = 30 AND group_no = 1);

-- 2) Seluruh juz yang sudah terisi masuk ke kelompok pembina Munflih
UPDATE khatmil_juz_assignments SET group_id = @g
WHERE campaign_id = 30 AND group_id IS NULL;

-- 3) Santri Uji ikut campaign 30
INSERT INTO khatmil_participants (campaign_id, user_id)
SELECT 30, 2 WHERE NOT EXISTS
  (SELECT 1 FROM khatmil_participants WHERE campaign_id = 30 AND user_id = 2);
SET @p2 := (SELECT id FROM khatmil_participants WHERE campaign_id = 30 AND user_id = 2);

-- 4) Santri Uji memegang juz 26 (IN_PROGRESS, di kelompok Munflih)
INSERT INTO khatmil_juz_assignments (campaign_id, participant_id, juz, status, group_id)
SELECT 30, @p2, 26, 'IN_PROGRESS', @g
WHERE NOT EXISTS (
  SELECT 1 FROM khatmil_juz_assignments
  WHERE campaign_id = 30 AND participant_id = @p2 AND active_marker = 1);
SET @a26 := (SELECT id FROM khatmil_juz_assignments
  WHERE campaign_id = 30 AND participant_id = @p2 AND active_marker = 1);

-- 5) Progres bacaan Santri Uji: juz 26 sudah 10 halaman, terakhir Al-Fath 27
INSERT INTO khatmil_progress (assignment_id, pages_read, minutes_read, current_surah_id, current_ayah, verification)
SELECT @a26, 10, 20, 48, 27, 'SELF_REPORTED'
WHERE @a26 IS NOT NULL AND NOT EXISTS
  (SELECT 1 FROM khatmil_progress WHERE assignment_id = @a26);

SELECT @g AS group_id, @p2 AS participant_santri_uji, @a26 AS assignment_juz26;
