-- =============================================================
-- MQ — 0019 (MySQL) Khatmil: kolom posisi bacaan Surah/Ayat
-- Plan khatmil admin rev 3.3 + mobile rev 3.3 (completion v2 POSISI-ONLY):
--   * khatmil_progress        = rollup posisi terkini (timpa, bukan GREATEST)
--   * khatmil_progress_events = riwayat laporan per posisi (audit)
-- pages_read/minutes_read tetap (telemetri legacy; events NOT NULL -> 0).
-- =============================================================

ALTER TABLE khatmil_progress
    ADD COLUMN current_surah_id INT NULL AFTER minutes_read,
    ADD COLUMN current_ayah     INT NULL AFTER current_surah_id;

ALTER TABLE khatmil_progress_events
    ADD COLUMN current_surah_id INT NULL AFTER minutes_read,
    ADD COLUMN current_ayah     INT NULL AFTER current_surah_id;

CREATE INDEX kpe_pos_idx ON khatmil_progress_events (assignment_id, current_surah_id, current_ayah);
