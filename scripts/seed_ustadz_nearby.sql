-- =============================================================
-- MQ — Seed data uji: 5 ustadz di sekitar lokasi santri test
-- (area -6.5399, 106.7790) dengan variasi hari, jam, tarif, blackout.
-- Idempotent: aman dijalankan berulang (ON DUPLICATE KEY).
-- Semua password = rahasia123 (disalin dari user santri.test id=2).
-- Weekday: 0=Minggu 1=Senin 2=Selasa 3=Rabu 4=Kamis 5=Jumat 6=Sabtu
-- minute: 16:00 = 960
-- =============================================================
SET time_zone = '+00:00';

-- ---------- helper: 1 user ustadz + profil + lokasi + setting ----------
-- dipanggil 5x dengan parameter berbeda di bawah

-- ============ 1. Ustadz Ahmad Fauzi (~400 m) ============
INSERT INTO users (email, password_hash, account_type, status, email_verified_at)
SELECT 'ustadz.ahmad.test@mq.local', u.password_hash, 'PENGURUS', 'ACTIVE', UTC_TIMESTAMP()
FROM users u WHERE u.id = 2
ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(users.id), status = 'ACTIVE';
SET @u1 := LAST_INSERT_ID();
INSERT INTO user_profiles (user_id, full_name, gender, city)
VALUES (@u1, 'Ustadz Ahmad Fauzi', 'MALE', 'Bogor')
ON DUPLICATE KEY UPDATE full_name = VALUES(full_name);
INSERT INTO ustadz_profiles (user_id, code, title, verified_at)
VALUES (@u1, 'ust-ahmad-test', 'Ustadz', UTC_TIMESTAMP())
ON DUPLICATE KEY UPDATE verified_at = IFNULL(verified_at, UTC_TIMESTAMP());
INSERT INTO user_locations (user_id, lat, lng, accuracy_m, recorded_at)
VALUES (@u1, -6.536400, 106.779000, 15, UTC_TIMESTAMP())
ON DUPLICATE KEY UPDATE lat = VALUES(lat), lng = VALUES(lng), recorded_at = UTC_TIMESTAMP();
INSERT INTO ustadz_visit_settings (ustadz_id, is_accepting, max_active_visits, price_per_hour)
VALUES (@u1, 1, 3, 15000)
ON DUPLICATE KEY UPDATE is_accepting = 1, price_per_hour = 15000;
DELETE FROM ustadz_availability_slots WHERE ustadz_id = @u1;
INSERT INTO ustadz_availability_slots (ustadz_id, weekday, start_minute, end_minute) VALUES
  (@u1, 1, 960, 1200),   -- Senin 16:00-20:00
  (@u1, 3, 960, 1200);   -- Rabu 16:00-20:00
DELETE FROM ustadz_blackout_dates WHERE ustadz_id = @u1;
INSERT INTO ustadz_blackout_dates (ustadz_id, off_date, note) VALUES
  (@u1, DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) + INTERVAL 3 DAY), 'Mengaji keluarga');

-- ============ 2. Ustadz Hasan Basri (~650 m) ============
INSERT INTO users (email, password_hash, account_type, status, email_verified_at)
SELECT 'ustadz.hasan.test@mq.local', u.password_hash, 'PENGURUS', 'ACTIVE', UTC_TIMESTAMP()
FROM users u WHERE u.id = 2
ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(users.id), status = 'ACTIVE';
SET @u2 := LAST_INSERT_ID();
INSERT INTO user_profiles (user_id, full_name, gender, city)
VALUES (@u2, 'Ustadz Hasan Basri', 'MALE', 'Bogor')
ON DUPLICATE KEY UPDATE full_name = VALUES(full_name);
INSERT INTO ustadz_profiles (user_id, code, title, verified_at)
VALUES (@u2, 'ust-hasan-test', 'Ustadz', UTC_TIMESTAMP())
ON DUPLICATE KEY UPDATE verified_at = IFNULL(verified_at, UTC_TIMESTAMP());
INSERT INTO user_locations (user_id, lat, lng, accuracy_m, recorded_at)
VALUES (@u2, -6.542900, 106.783000, 20, UTC_TIMESTAMP())
ON DUPLICATE KEY UPDATE lat = VALUES(lat), lng = VALUES(lng), recorded_at = UTC_TIMESTAMP();
INSERT INTO ustadz_visit_settings (ustadz_id, is_accepting, max_active_visits, price_per_hour)
VALUES (@u2, 1, 2, 10000)
ON DUPLICATE KEY UPDATE is_accepting = 1, price_per_hour = 10000;
DELETE FROM ustadz_availability_slots WHERE ustadz_id = @u2;
INSERT INTO ustadz_availability_slots (ustadz_id, weekday, start_minute, end_minute) VALUES
  (@u2, 2, 480, 660),    -- Selasa 08:00-11:00
  (@u2, 4, 780, 960);    -- Kamis 13:00-16:00

-- ============ 3. Ustadz Abdullah Syafiq (~1,1 km) ============
INSERT INTO users (email, password_hash, account_type, status, email_verified_at)
SELECT 'ustadz.syafiq.test@mq.local', u.password_hash, 'PENGURUS', 'ACTIVE', UTC_TIMESTAMP()
FROM users u WHERE u.id = 2
ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(users.id), status = 'ACTIVE';
SET @u3 := LAST_INSERT_ID();
INSERT INTO user_profiles (user_id, full_name, gender, city)
VALUES (@u3, 'Ustadz Abdullah Syafiq', 'MALE', 'Bogor')
ON DUPLICATE KEY UPDATE full_name = VALUES(full_name);
INSERT INTO ustadz_profiles (user_id, code, title, verified_at)
VALUES (@u3, 'ust-syafiq-test', 'Ustadz', UTC_TIMESTAMP())
ON DUPLICATE KEY UPDATE verified_at = IFNULL(verified_at, UTC_TIMESTAMP());
INSERT INTO user_locations (user_id, lat, lng, accuracy_m, recorded_at)
VALUES (@u3, -6.549900, 106.776000, 25, UTC_TIMESTAMP())
ON DUPLICATE KEY UPDATE lat = VALUES(lat), lng = VALUES(lng), recorded_at = UTC_TIMESTAMP();
INSERT INTO ustadz_visit_settings (ustadz_id, is_accepting, max_active_visits, price_per_hour)
VALUES (@u3, 1, 2, 20000)
ON DUPLICATE KEY UPDATE is_accepting = 1, price_per_hour = 20000;
DELETE FROM ustadz_availability_slots WHERE ustadz_id = @u3;
INSERT INTO ustadz_availability_slots (ustadz_id, weekday, start_minute, end_minute) VALUES
  (@u3, 6, 540, 720),    -- Sabtu 09:00-12:00
  (@u3, 6, 840, 1020);   -- Sabtu 14:00-17:00

-- ============ 4. Ustadz Ibrahim Musa (~1,8 km) ============
INSERT INTO users (email, password_hash, account_type, status, email_verified_at)
SELECT 'ustadz.ibrahim.test@mq.local', u.password_hash, 'PENGURUS', 'ACTIVE', UTC_TIMESTAMP()
FROM users u WHERE u.id = 2
ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(users.id), status = 'ACTIVE';
SET @u4 := LAST_INSERT_ID();
INSERT INTO user_profiles (user_id, full_name, gender, city)
VALUES (@u4, 'Ustadz Ibrahim Musa', 'MALE', 'Bogor')
ON DUPLICATE KEY UPDATE full_name = VALUES(full_name);
INSERT INTO ustadz_profiles (user_id, code, title, verified_at)
VALUES (@u4, 'ust-ibrahim-test', 'Ustadz', UTC_TIMESTAMP())
ON DUPLICATE KEY UPDATE verified_at = IFNULL(verified_at, UTC_TIMESTAMP());
INSERT INTO user_locations (user_id, lat, lng, accuracy_m, recorded_at)
VALUES (@u4, -6.547900, 106.794000, 30, UTC_TIMESTAMP())
ON DUPLICATE KEY UPDATE lat = VALUES(lat), lng = VALUES(lng), recorded_at = UTC_TIMESTAMP();
INSERT INTO ustadz_visit_settings (ustadz_id, is_accepting, max_active_visits, price_per_hour)
VALUES (@u4, 1, 4, 12000)
ON DUPLICATE KEY UPDATE is_accepting = 1, price_per_hour = 12000;
DELETE FROM ustadz_availability_slots WHERE ustadz_id = @u4;
INSERT INTO ustadz_availability_slots (ustadz_id, weekday, start_minute, end_minute) VALUES
  (@u4, 5, 420, 600),    -- Jumat 07:00-10:00
  (@u4, 0, 960, 1140);   -- Minggu 16:00-19:00

-- ============ 5. Ustadz Yusuf Hamdani (~2,2 km) ============
INSERT INTO users (email, password_hash, account_type, status, email_verified_at)
SELECT 'ustadz.yusuf.test@mq.local', u.password_hash, 'PENGURUS', 'ACTIVE', UTC_TIMESTAMP()
FROM users u WHERE u.id = 2
ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(users.id), status = 'ACTIVE';
SET @u5 := LAST_INSERT_ID();
INSERT INTO user_profiles (user_id, full_name, gender, city)
VALUES (@u5, 'Ustadz Yusuf Hamdani', 'MALE', 'Bogor')
ON DUPLICATE KEY UPDATE full_name = VALUES(full_name);
INSERT INTO ustadz_profiles (user_id, code, title, verified_at)
VALUES (@u5, 'ust-yusuf-test', 'Ustadz', UTC_TIMESTAMP())
ON DUPLICATE KEY UPDATE verified_at = IFNULL(verified_at, UTC_TIMESTAMP());
INSERT INTO user_locations (user_id, lat, lng, accuracy_m, recorded_at)
VALUES (@u5, -6.519900, 106.780000, 30, UTC_TIMESTAMP())
ON DUPLICATE KEY UPDATE lat = VALUES(lat), lng = VALUES(lng), recorded_at = UTC_TIMESTAMP();
INSERT INTO ustadz_visit_settings (ustadz_id, is_accepting, max_active_visits, price_per_hour)
VALUES (@u5, 1, 3, 18000)
ON DUPLICATE KEY UPDATE is_accepting = 1, price_per_hour = 18000;
DELETE FROM ustadz_availability_slots WHERE ustadz_id = @u5;
INSERT INTO ustadz_availability_slots (ustadz_id, weekday, start_minute, end_minute) VALUES
  (@u5, 1, 480, 660),    -- Senin 08:00-11:00
  (@u5, 3, 1140, 1260),  -- Rabu 19:00-21:00
  (@u5, 5, 960, 1080);   -- Jumat 16:00-18:00
DELETE FROM ustadz_blackout_dates WHERE ustadz_id = @u5;
INSERT INTO ustadz_blackout_dates (ustadz_id, off_date, note) VALUES
  (@u5, DATE((UTC_TIMESTAMP() + INTERVAL 7 HOUR) + INTERVAL 1 DAY), 'Acara keluarga');

SELECT 'seed selesai' AS status;
