-- Perbaikan data uji: posisi bacaan seed yang tidak konsisten dengan rentang juz
-- (menyebabkan juz terbaca 100% palsu). Posisi baru = ayat ke-31 dari tiap juz.
UPDATE khatmil_progress pr
JOIN khatmil_juz_assignments a ON a.id = pr.assignment_id
JOIN khatmil_campaigns c ON c.id = a.campaign_id
SET pr.current_surah_id = (SELECT q.surah_id FROM quran_ayahs q WHERE q.juz = a.juz ORDER BY q.surah_id, q.ayah_number LIMIT 1 OFFSET 30),
    pr.current_ayah = (SELECT q.ayah_number FROM quran_ayahs q WHERE q.juz = a.juz ORDER BY q.surah_id, q.ayah_number LIMIT 1 OFFSET 30)
WHERE c.slug IN ('status-act-1','status-act-2') AND a.status = 'IN_PROGRESS';
