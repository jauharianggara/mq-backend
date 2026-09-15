-- =============================================================
-- MQ — 0019b (MySQL) Khatmil: relax CHECK events (rev 3.3 posisi-only)
-- pages_read/minutes_read kini OPSIONAL (telemetri legacy, default 0)
-- → constraint events "> 0" dilonggarkan jadi ">= 0"
-- (khatmil_progress sudah 0..N — tidak berubah)
-- =============================================================

ALTER TABLE khatmil_progress_events DROP CHECK kge_pages_chk;
ALTER TABLE khatmil_progress_events DROP CHECK kge_minutes_chk;
ALTER TABLE khatmil_progress_events ADD CONSTRAINT kge_pages_chk CHECK (pages_read >= 0);
ALTER TABLE khatmil_progress_events ADD CONSTRAINT kge_minutes_chk CHECK (minutes_read >= 0);
