-- =============================================================
-- MQ Digital Platform — 0020 (MySQL) Khatmil v2: kelompok + penugasan pembina
-- (plan 2026-09-15_mq-khatmil-v2.md rev 4 + 2026-09-15_mq-sederhanakan-app.md)
--  * khatmil_groups : kelompok eksplisit per campaign (group_no 1..group_count)
--    pembina = ustadz; penugasan dua langkah: admin isi pending_ustadz_id ->
--    ustadz ACC di app -> ustadz_id terisi (dua langkah, ada jejak)
--  * khatmil_campaigns.group_count : jumlah kelompok (pengganti semantik
--    target_khataman; kolom lama TIDAK dihapus — data arsip tetap terbaca)
--  * khatmil_juz_assignments.group_id : relasi eksplisit assignment -> kelompok
--    (NULL = assignment legacy sebelum model kelompok; dianggap arsip)
-- =============================================================

ALTER TABLE khatmil_campaigns
  ADD COLUMN group_count SMALLINT NOT NULL DEFAULT 1 AFTER target_khataman;

CREATE TABLE khatmil_groups (
  id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
  campaign_id BIGINT NOT NULL,
  group_no SMALLINT NOT NULL,          -- 1..group_count
  ustadz_id BIGINT NULL,               -- pembina resmi (setelah ACC ustadz)
  pending_ustadz_id BIGINT NULL,       -- penugasan menunggu ACC ustadz
  pending_at TIMESTAMP NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE KEY kg_campaign_group_uq (campaign_id, group_no),
  CONSTRAINT kg_campaign_fk FOREIGN KEY (campaign_id) REFERENCES khatmil_campaigns (id) ON DELETE CASCADE,
  CONSTRAINT kg_ustadz_fk FOREIGN KEY (ustadz_id) REFERENCES users (id) ON DELETE SET NULL,
  CONSTRAINT kg_pending_fk FOREIGN KEY (pending_ustadz_id) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB;

ALTER TABLE khatmil_juz_assignments
  ADD COLUMN group_id BIGINT NULL,
  ADD KEY kja_group_idx (group_id);

ALTER TABLE khatmil_juz_assignments
  ADD CONSTRAINT kja_group_fk FOREIGN KEY (group_id) REFERENCES khatmil_groups (id) ON DELETE SET NULL;

-- notif templates penugasan (idempotent by unique code)
INSERT IGNORE INTO notification_templates (code, title_template, body_template, default_channel)
VALUES
  ('KHATMIL_ASSIGN_REQUEST', 'Penugasan Pembina Khatmil',
   'Anda ditugaskan menjadi Pembina khatmil "{{campaign}}" Kelompok {{group_no}}. Buka menu Khatmil untuk menerima atau menolak.', 'IN_APP'),
  ('KHATMIL_ASSIGN_RESULT', 'Hasil penugasan pembina',
   'Ustadz {{ustadz}} telah menjawab penugasan Pembina khatmil "{{campaign}}" Kelompok {{group_no}}: {{result}}.', 'IN_APP');
