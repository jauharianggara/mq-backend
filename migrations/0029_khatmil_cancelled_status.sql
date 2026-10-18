-- 0029: status CANCELLED untuk penugasan juz — peserta boleh melepas juz yang
-- dipegangnya selama khotmil belum Selesai (keputusan produk 2026-10-18 #4).
-- CANCELLED membuat active_marker NULL → juz kembali kosong di peta.
ALTER TABLE khatmil_juz_assignments DROP CHECK ka_status_chk;
ALTER TABLE khatmil_juz_assignments
    ADD CONSTRAINT ka_status_chk CHECK (status IN ('ASSIGNED','IN_PROGRESS','COMPLETED','EXPIRED','REASSIGNED','CANCELLED'));
