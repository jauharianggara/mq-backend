-- =============================================================
-- MQ — 0024 Titik Lokasi Rujukan
-- - user_home_points : titik default santri (1:1 user)
-- - ustadz_profiles  : satu titik lokasi ustadz (tanpa label kategori)
-- GPS real-time ustadz berhenti dipakai untuk keterlihatan (nearby).
-- Idempotent.
-- =============================================================

CREATE TABLE IF NOT EXISTS user_home_points (
  user_id BIGINT NOT NULL PRIMARY KEY,
  lat DECIMAL(9,6) NOT NULL,
  lng DECIMAL(9,6) NOT NULL,
  address_label VARCHAR(200) NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- kolom titik lokasi ustadz
SET @c := (SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema=DATABASE() AND table_name='ustadz_profiles' AND column_name='point_lat');
SET @sql := IF(@c=0, 'ALTER TABLE ustadz_profiles ADD COLUMN point_lat DECIMAL(9,6) NULL AFTER pengalaman_mengajar', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

SET @c := (SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema=DATABASE() AND table_name='ustadz_profiles' AND column_name='point_lng');
SET @sql := IF(@c=0, 'ALTER TABLE ustadz_profiles ADD COLUMN point_lng DECIMAL(9,6) NULL AFTER point_lat', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

SET @c := (SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema=DATABASE() AND table_name='ustadz_profiles' AND column_name='point_label');
SET @sql := IF(@c=0, 'ALTER TABLE ustadz_profiles ADD COLUMN point_label VARCHAR(120) NULL AFTER point_lng', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

-- data uji: pindahkan titik ustadz yang sudah ada di user_locations ke titik profil
UPDATE ustadz_profiles up
JOIN user_locations ul ON ul.user_id = up.user_id
SET up.point_lat = ul.lat, up.point_lng = ul.lng, up.point_label = 'Titik lokasi saya'
WHERE up.point_lat IS NULL AND up.point_lng IS NULL;

UPDATE ustadz_profiles up
JOIN ustadz_visit_settings vs ON vs.ustadz_id = up.user_id
SET up.point_lat = -6.5385, up.point_lng = 106.7785, up.point_label = 'Titik lokasi saya'
WHERE up.user_id = 415 AND up.point_lat IS NULL;
