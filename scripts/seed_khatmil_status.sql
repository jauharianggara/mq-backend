-- =============================================================
-- MQ — Seed contoh KHATMIL per STATUS (2 khatamil per status)
-- Draf 2 · Akan Datang 2 · Berlangsung 2 · Selesai 2
-- Idempotent: aman dijalankan berulang.
-- Pembina: 415 Ahmad Dahlan Munflih · 5 Ustadz Test · 309 Ahmad Dahlan Muflih
-- =============================================================
SET time_zone = '+00:00';

-- ---------- 1. DRAFT ----------
INSERT INTO khatmil_campaigns (slug, name, description, mode, status, target_khataman, group_count, min_minutes_per_juz, created_by)
VALUES
 ('status-draft-1', 'Khatamil Persiapan Batch 3', 'Sedang disiapkan pengelola.', 'PARALLEL', 'DRAFT', 1, 1, 30, 1),
 ('status-draft-2', 'Khatamil Khusus Tahfidz', 'Belum dibuka untuk santri.', 'PARALLEL', 'DRAFT', 1, 1, 30, 1)
AS c
ON DUPLICATE KEY UPDATE name = c.name, description = c.description;

-- ---------- 2. SCHEDULED ----------
INSERT INTO khatmil_campaigns (slug, name, description, mode, status, target_khataman, group_count, min_minutes_per_juz, period_start, period_end, created_by)
VALUES
 ('status-sch-1', 'Khatamil Ramadan 1448 H', 'Khatamil bersama menyambut Ramadan.', 'PARALLEL', 'SCHEDULED', 1, 2, 30,
   DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) + INTERVAL 35 DAY), DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) + INTERVAL 125 DAY), 1),
 ('status-sch-2', 'Khatamil Semester Depan', 'Dibuka untuk santri semester depan.', 'PARALLEL', 'SCHEDULED', 1, 1, 30,
   DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) + INTERVAL 60 DAY), DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) + INTERVAL 150 DAY), 1)
AS c
ON DUPLICATE KEY UPDATE name = c.name, status = 'SCHEDULED';

-- ---------- 3. ACTIVE ----------
INSERT INTO khatmil_campaigns (slug, name, description, mode, status, target_khataman, group_count, min_minutes_per_juz, period_start, period_end, created_by)
VALUES
 ('status-act-1', 'Khatamil MQ Angkatan 1 2026', 'Khatamil utama santri MQ angkatan ini.', 'PARALLEL', 'ACTIVE', 1, 1, 30,
   DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) - INTERVAL 10 DAY), DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) + INTERVAL 80 DAY), 1),
 ('status-act-2', 'Khatamil Cepat 30 Hari', 'Khatamil intensif 30 hari.', 'PARALLEL', 'ACTIVE', 1, 1, 30,
   DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) - INTERVAL 3 DAY), DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) + INTERVAL 27 DAY), 1)
AS c
ON DUPLICATE KEY UPDATE name = c.name, status = 'ACTIVE';

-- ---------- 4. COMPLETED ----------
INSERT INTO khatmil_campaigns (slug, name, description, mode, status, target_khataman, group_count, min_minutes_per_juz, period_start, period_end, created_by)
VALUES
 ('status-done-1', 'Khatamil Angkatan 12 2025', 'Riwayat khatamil angkatan 12.', 'PARALLEL', 'COMPLETED', 1, 1, 30,
   DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) - INTERVAL 100 DAY), DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) - INTERVAL 20 DAY), 1),
 ('status-done-2', 'Khatamil Liburan 2025', 'Riwayat khatamil masa liburan.', 'PARALLEL', 'COMPLETED', 1, 1, 30,
   DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) - INTERVAL 120 DAY), DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) - INTERVAL 40 DAY), 1)
AS c
ON DUPLICATE KEY UPDATE name = c.name, status = 'COMPLETED';

-- =============================================================
-- Helper peserta: ambil user dummy @mq-demo.id yang belum ikut campaign tsb
-- =============================================================

-- ---------- isi peserta & kelompok SCHEDULED 1 (2 kelompok, tanpa juz) ----------
SET @c := (SELECT id FROM khatmil_campaigns WHERE slug = 'status-sch-1');
INSERT IGNORE INTO khatmil_groups (campaign_id, group_no, ustadz_id)
  VALUES (@c, 1, 415), (@c, 2, 5);
INSERT INTO khatmil_participants (campaign_id, user_id)
  SELECT @c, u.id FROM users u
  WHERE u.email LIKE '%@mq-demo.id' AND u.status = 'ACTIVE'
    AND NOT EXISTS (SELECT 1 FROM khatmil_participants p WHERE p.campaign_id = @c AND p.user_id = u.id)
  ORDER BY u.id LIMIT 10;

-- ---------- SCHEDULED 2 ----------
SET @c := (SELECT id FROM khatmil_campaigns WHERE slug = 'status-sch-2');
INSERT IGNORE INTO khatmil_groups (campaign_id, group_no, ustadz_id)
  VALUES (@c, 1, 309);
INSERT INTO khatmil_participants (campaign_id, user_id)
  SELECT @c, u.id FROM users u
  WHERE u.email LIKE '%@mq-demo.id' AND u.status = 'ACTIVE'
    AND NOT EXISTS (SELECT 1 FROM khatmil_participants p WHERE p.campaign_id = @c AND p.user_id = u.id)
  ORDER BY u.id LIMIT 8;

-- ---------- ACTIVE 1: 12 peserta, juz 1-12 IN_PROGRESS, kelompok 1 (Dahlan) ----------
SET @c := (SELECT id FROM khatmil_campaigns WHERE slug = 'status-act-1');
SET @g := (SELECT g.id FROM khatmil_groups g JOIN khatmil_campaigns c ON c.id = g.campaign_id
           WHERE c.slug = 'status-act-1' AND g.group_no = 1);
INSERT IGNORE INTO khatmil_groups (campaign_id, group_no, ustadz_id)
  VALUES (@c, 1, 415);
INSERT INTO khatmil_participants (campaign_id, user_id)
  SELECT @c, u.id FROM users u
  WHERE u.email LIKE '%@mq-demo.id' AND u.status = 'ACTIVE'
    AND NOT EXISTS (SELECT 1 FROM khatmil_participants p WHERE p.campaign_id = @c AND p.user_id = u.id)
  ORDER BY u.id LIMIT 12;
INSERT INTO khatmil_juz_assignments (campaign_id, participant_id, juz, status, group_id)
  SELECT @c, p.id,
         ROW_NUMBER() OVER (ORDER BY p.id), 'IN_PROGRESS', @g
  FROM (SELECT id FROM khatmil_participants WHERE campaign_id = @c ORDER BY id LIMIT 12) p
  WHERE NOT EXISTS (SELECT 1 FROM khatmil_juz_assignments a
                    WHERE a.campaign_id = @c AND a.participant_id = p.id AND a.active_marker IS NOT NULL);
INSERT INTO khatmil_progress (assignment_id, pages_read, minutes_read, current_surah_id, current_ayah, verification)
  SELECT a.id, 5 + (a.juz MOD 4) * 3, 15 + (a.juz MOD 4) * 5, a.juz + 1, (a.juz * 37) MOD 120 + 5, 'SELF_REPORTED'
  FROM khatmil_juz_assignments a
  WHERE a.campaign_id = @c AND a.active_marker IS NOT NULL
    AND NOT EXISTS (SELECT 1 FROM khatmil_progress pr WHERE pr.assignment_id = a.id);

-- ---------- ACTIVE 2: 8 peserta, juz 1-8 IN_PROGRESS, kelompok 1 (Ustadz Test) ----------
SET @c := (SELECT id FROM khatmil_campaigns WHERE slug = 'status-act-2');
INSERT IGNORE INTO khatmil_groups (campaign_id, group_no, ustadz_id)
  VALUES (@c, 1, 5);
INSERT INTO khatmil_participants (campaign_id, user_id)
  SELECT @c, u.id FROM users u
  WHERE u.email LIKE '%@mq-demo.id' AND u.status = 'ACTIVE'
    AND NOT EXISTS (SELECT 1 FROM khatmil_participants p WHERE p.campaign_id = @c AND p.user_id = u.id)
  ORDER BY u.id LIMIT 8;
INSERT INTO khatmil_juz_assignments (campaign_id, participant_id, juz, status, group_id)
  SELECT @c, p.id,
         ROW_NUMBER() OVER (ORDER BY p.id), 'IN_PROGRESS', @g2
  FROM (SELECT id FROM khatmil_participants WHERE campaign_id = @c ORDER BY id LIMIT 8) p
  WHERE NOT EXISTS (SELECT 1 FROM khatmil_juz_assignments a
                    WHERE a.campaign_id = @c AND a.participant_id = p.id AND a.active_marker IS NOT NULL);
SET @g2 := (SELECT g.id FROM khatmil_groups g JOIN khatmil_campaigns c ON c.id = g.campaign_id
            WHERE c.slug = 'status-act-2' AND g.group_no = 1);
UPDATE khatmil_juz_assignments a JOIN khatmil_campaigns c ON c.id = a.campaign_id
  SET a.group_id = @g2 WHERE c.slug = 'status-act-2' AND a.group_id IS NULL;
INSERT INTO khatmil_progress (assignment_id, pages_read, minutes_read, current_surah_id, current_ayah, verification)
  SELECT a.id, 3 + (a.juz MOD 3) * 2, 10, a.juz + 1, (a.juz * 41) MOD 100 + 3, 'SELF_REPORTED'
  FROM khatmil_juz_assignments a
  WHERE a.campaign_id = @c AND a.active_marker IS NOT NULL
    AND NOT EXISTS (SELECT 1 FROM khatmil_progress pr WHERE pr.assignment_id = a.id);

-- ---------- COMPLETED 1 & 2: 30 peserta, 30 juz COMPLETED ----------
SET @c := (SELECT id FROM khatmil_campaigns WHERE slug = 'status-done-1');
INSERT IGNORE INTO khatmil_groups (campaign_id, group_no, ustadz_id) VALUES (@c, 1, 5);
INSERT INTO khatmil_participants (campaign_id, user_id)
  SELECT @c, u.id FROM users u
  WHERE u.email LIKE '%@mq-demo.id' AND u.status = 'ACTIVE'
    AND NOT EXISTS (SELECT 1 FROM khatmil_participants p WHERE p.campaign_id = @c AND p.user_id = u.id)
  ORDER BY u.id LIMIT 30;
INSERT INTO khatmil_juz_assignments (campaign_id, participant_id, juz, status, group_id)
  SELECT @c, p.id,
         ROW_NUMBER() OVER (ORDER BY p.id), 'COMPLETED', @g
  FROM (SELECT id FROM khatmil_participants WHERE campaign_id = @c ORDER BY id LIMIT 30) p
  WHERE NOT EXISTS (SELECT 1 FROM khatmil_juz_assignments a
                    WHERE a.campaign_id = @c AND a.participant_id = p.id AND a.active_marker IS NOT NULL);
SET @g := (SELECT g.id FROM khatmil_groups g JOIN khatmil_campaigns c ON c.id = g.campaign_id
           WHERE c.slug = 'status-done-1' AND g.group_no = 1);
UPDATE khatmil_juz_assignments a JOIN khatmil_campaigns c ON c.id = a.campaign_id
  SET a.group_id = @g WHERE c.slug = 'status-done-1' AND a.group_id IS NULL;
INSERT INTO khatmil_progress (assignment_id, pages_read, minutes_read, current_surah_id, current_ayah, verification)
  SELECT a.id, 20, 40, NULL, NULL, 'SELF_REPORTED'
  FROM khatmil_juz_assignments a
  WHERE a.campaign_id = @c AND a.active_marker IS NOT NULL
    AND NOT EXISTS (SELECT 1 FROM khatmil_progress pr WHERE pr.assignment_id = a.id);
INSERT IGNORE INTO khatmil_completions (campaign_id, cycle)
  SELECT @c, 1 WHERE NOT EXISTS (SELECT 1 FROM khatmil_completions WHERE campaign_id = @c);

SET @c := (SELECT id FROM khatmil_campaigns WHERE slug = 'status-done-2');
INSERT IGNORE INTO khatmil_groups (campaign_id, group_no, ustadz_id) VALUES (@c, 1, 415);
INSERT INTO khatmil_participants (campaign_id, user_id)
  SELECT @c, u.id FROM users u
  WHERE u.email LIKE '%@mq-demo.id' AND u.status = 'ACTIVE'
    AND NOT EXISTS (SELECT 1 FROM khatmil_participants p WHERE p.campaign_id = @c AND p.user_id = u.id)
  ORDER BY u.id LIMIT 30;
INSERT INTO khatmil_juz_assignments (campaign_id, participant_id, juz, status, group_id)
  SELECT @c, p.id,
         ROW_NUMBER() OVER (ORDER BY p.id), 'COMPLETED', @g
  FROM (SELECT id FROM khatmil_participants WHERE campaign_id = @c ORDER BY id LIMIT 30) p
  WHERE NOT EXISTS (SELECT 1 FROM khatmil_juz_assignments a
                    WHERE a.campaign_id = @c AND a.participant_id = p.id AND a.active_marker IS NOT NULL);
SET @g := (SELECT g.id FROM khatmil_groups g JOIN khatmil_campaigns c ON c.id = g.campaign_id
           WHERE c.slug = 'status-done-2' AND g.group_no = 1);
UPDATE khatmil_juz_assignments a JOIN khatmil_campaigns c ON c.id = a.campaign_id
  SET a.group_id = @g WHERE c.slug = 'status-done-2' AND a.group_id IS NULL;
INSERT INTO khatmil_progress (assignment_id, pages_read, minutes_read, current_surah_id, current_ayah, verification)
  SELECT a.id, 20, 38, NULL, NULL, 'SELF_REPORTED'
  FROM khatmil_juz_assignments a
  WHERE a.campaign_id = @c AND a.active_marker IS NOT NULL
    AND NOT EXISTS (SELECT 1 FROM khatmil_progress pr WHERE pr.assignment_id = a.id);
INSERT IGNORE INTO khatmil_completions (campaign_id, cycle)
  SELECT @c, 1 WHERE NOT EXISTS (SELECT 1 FROM khatmil_completions WHERE campaign_id = @c);

-- ---------- ringkasan ----------
SELECT c.slug, c.name, c.status, c.group_count,
       (SELECT COUNT(*) FROM khatmil_groups g WHERE g.campaign_id = c.id) kelompok,
       (SELECT COUNT(*) FROM khatmil_juz_assignments a WHERE a.campaign_id = c.id AND a.active_marker IS NOT NULL) juz_terisi
FROM khatmil_campaigns c WHERE c.slug LIKE 'status-%' ORDER BY c.slug;
