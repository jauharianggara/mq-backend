-- =============================================================
-- MQ Digital Platform — 0017 (MySQL) ustadz visits ("Pesan Ustadz")
-- (Bagian V rev 6 — on-demand kunjungan ustadz + GPS + rating dua arah)
--  * visit_service_types : master jenis layanan (seed 4)
--  * user_locations      : snapshot lokasi terakhir per user (freshness nearby)
--  * ustadz_visit_settings: opt-in ustadz (radius GLOBAL di settings, bukan sini)
--  * ustadz_visit_services: tarif per layanan (WAJIB > 0 — tidak ada booking gratis)
--  * ustadz_visits       : booking + active_marker (max 1 aktif/santri DB-enforced)
--                          + client_key UNIQUE (idempotency booking)
--  * status_history      : audit transisi (pola question_status_history)
--  * visit_messages      : chat dalam app saat CONFIRMED (text-only)
--  * visit_reviews       : rating DUA ARAH double-blind (revealed_at/hidden)
-- =============================================================

CREATE TABLE visit_service_types (
    id          TINYINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    code        VARCHAR(30)  NOT NULL UNIQUE,   -- tahsin_privat | murajaah | tahlil_yasinan | konsultasi
    name        VARCHAR(100) NOT NULL,
    description VARCHAR(255) NULL,
    sort_order  SMALLINT NOT NULL DEFAULT 0,
    active      TINYINT(1) NOT NULL DEFAULT 1
) ENGINE=InnoDB;

CREATE TABLE user_locations (
    id          BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id     BIGINT NOT NULL UNIQUE,         -- 1 row per user (upsert)
    lat         DECIMAL(9,6) NOT NULL,
    lng         DECIMAL(9,6) NOT NULL,
    accuracy_m  SMALLINT NULL,
    recorded_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,   -- freshness utk nearby
    updated_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT ul_user_fk  FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT ul_lat_chk  CHECK (lat BETWEEN -90 AND 90),
    CONSTRAINT ul_lng_chk  CHECK (lng BETWEEN -180 AND 180),
    KEY ul_lat_lng_idx (lat, lng)
) ENGINE=InnoDB;

CREATE TABLE ustadz_visit_settings (
    ustadz_id        BIGINT NOT NULL PRIMARY KEY,   -- radius TIDAK di sini: global settings visit_radius_km (admin)
    is_accepting     TINYINT(1) NOT NULL DEFAULT 0,
    max_active_visits TINYINT NOT NULL DEFAULT 2,
    updated_at       TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT uvs_ustadz_fk FOREIGN KEY (ustadz_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT uvs_max_chk CHECK (max_active_visits BETWEEN 1 AND 10)
) ENGINE=InnoDB;

CREATE TABLE ustadz_visit_services (
    id               BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    ustadz_id        BIGINT NOT NULL,
    service_type_id  TINYINT NOT NULL,
    price_amount     BIGINT NOT NULL,             -- IDR, WAJIB > 0; tarif ditentukan ustadz
    duration_minutes SMALLINT NOT NULL DEFAULT 60,
    note             VARCHAR(255) NULL,
    active           TINYINT(1) NOT NULL DEFAULT 1,
    updated_at       TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT uvsr_pk        UNIQUE (ustadz_id, service_type_id),
    CONSTRAINT uvsr_ustadz_fk FOREIGN KEY (ustadz_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT uvsr_type_fk   FOREIGN KEY (service_type_id) REFERENCES visit_service_types (id),
    CONSTRAINT uvsr_price_chk CHECK (price_amount >= 1000 AND price_amount <= 100000000)   -- WAJIB berbayar (user 15Sep); min final = min Xendit (V0)
) ENGINE=InnoDB;

CREATE TABLE ustadz_visits (
    id               BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id          BIGINT NOT NULL,             -- santri pemesan
    ustadz_id        BIGINT NOT NULL,
    service_type_id  TINYINT NOT NULL,
    scheduled_at     TIMESTAMP NOT NULL,           -- UTC (pool time_zone +00:00)
    duration_minutes SMALLINT NOT NULL DEFAULT 60,
    lat              DECIMAL(9,6) NOT NULL,        -- titik santri saat pesan
    lng              DECIMAL(9,6) NOT NULL,
    address_label    VARCHAR(100) NOT NULL,        -- patokan rumah (WAJIB, teks bebas)
    note             VARCHAR(500) NULL,
    client_key       VARCHAR(64) NULL,             -- idempotency booking (header Idempotency-Key)
    price_amount     BIGINT NOT NULL,              -- snapshot tarif saat booking (>0 — semua booking berbayar)
    anonymized       TINYINT(1) NOT NULL DEFAULT 0, -- PDP: lokasi pemesan sudah dihapus (lat/lng=(0,0) + label '—')
    platform_fee     BIGINT NOT NULL DEFAULT 0,
    status           VARCHAR(20) NOT NULL DEFAULT 'REQUESTED',
    active_marker    TINYINT GENERATED ALWAYS AS (
                         CASE WHEN status IN ('REQUESTED','WAITING_CONFIRM','CONFIRMED') THEN 1 ELSE NULL END
                     ) STORED,                      -- max 1 booking aktif per santri (DB-enforced)
    decline_reason   VARCHAR(255) NULL,
    cancel_reason    VARCHAR(255) NULL,
    canceled_by      BIGINT NULL,                  -- user_id pemicu cancel (santri/admin)
    created_at       TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    paid_at          TIMESTAMP NULL,
    confirmed_at     TIMESTAMP NULL,
    declined_at      TIMESTAMP NULL,
    canceled_at      TIMESTAMP NULL,
    completed_at     TIMESTAMP NULL,
    reviewed_at      TIMESTAMP NULL,
    CONSTRAINT uv_user_fk   FOREIGN KEY (user_id)   REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT uv_ustadz_fk FOREIGN KEY (ustadz_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT uv_type_fk   FOREIGN KEY (service_type_id) REFERENCES visit_service_types (id),
    CONSTRAINT uv_active_uq UNIQUE (user_id, active_marker),   -- << DB-enforced max 1 booking aktif per santri
    CONSTRAINT uv_client_uq UNIQUE (user_id, client_key),      -- << DB-enforced dedupe retry (idempotency booking)
    CONSTRAINT uv_status_chk CHECK (status IN ('REQUESTED','WAITING_CONFIRM','CONFIRMED',
        'DECLINED','CANCELED','PAYMENT_EXPIRED','COMPLETED','REVIEWED')),
    KEY uv_ustadz_idx (ustadz_id, status),
    KEY uv_sched_idx (scheduled_at),
    KEY uv_ustadz_sched_idx (ustadz_id, status, scheduled_at),
    KEY uv_worker_idx (status, completed_at)      -- utk worker review_window (COMPLETED lewat window)
) ENGINE=InnoDB;

CREATE TABLE ustadz_visit_status_history (
    id          BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    visit_id    BIGINT NOT NULL,
    from_status VARCHAR(20) NULL,
    to_status   VARCHAR(20) NOT NULL,
    actor_id    BIGINT NULL,
    note        VARCHAR(255) NULL,
    created_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uvh_visit_fk FOREIGN KEY (visit_id) REFERENCES ustadz_visits (id) ON DELETE CASCADE,
    KEY uvh_visit_idx (visit_id, id)
) ENGINE=InnoDB;

CREATE TABLE visit_messages (
    id         BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    visit_id   BIGINT NOT NULL,
    sender_id  BIGINT NOT NULL,                   -- santri atau ustadz (hanya peserta visit; gate di service)
    body       VARCHAR(1000) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    read_at    TIMESTAMP NULL,                    -- diisi saat penerima fetch -> badge unread
    CONSTRAINT vm_visit_fk   FOREIGN KEY (visit_id)  REFERENCES ustadz_visits (id) ON DELETE CASCADE,
    CONSTRAINT vm_sender_fk  FOREIGN KEY (sender_id) REFERENCES users (id) ON DELETE CASCADE,
    KEY vm_visit_idx (visit_id, id),
    KEY vm_visit_unread_idx (visit_id, read_at, id)
) ENGINE=InnoDB;

CREATE TABLE visit_reviews (
    id          BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    visit_id    BIGINT NOT NULL,
    direction   VARCHAR(20) NOT NULL,             -- SANTRI_TO_USTADZ | USTADZ_TO_SANTRI
    reviewer_id BIGINT NOT NULL,
    reviewee_id BIGINT NOT NULL,                  -- denormalisasi utk agregat rating profil (AVG by reviewee)
    rating      TINYINT NOT NULL,
    comment     VARCHAR(500) NULL,
    hidden      TINYINT(1) NOT NULL DEFAULT 0,    -- admin hide (abuse) — tidak tampil, tetap utk audit
    revealed_at TIMESTAMP NULL,                   -- NULL = belum terlihat (double-blind: tunggu 2 arah / window)
    created_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT vr_unique_dir UNIQUE (visit_id, direction),  -- 1x per arah per booking
    CONSTRAINT vr_visit_fk    FOREIGN KEY (visit_id)    REFERENCES ustadz_visits (id) ON DELETE CASCADE,
    CONSTRAINT vr_reviewer_fk FOREIGN KEY (reviewer_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT vr_reviewee_fk FOREIGN KEY (reviewee_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT vr_dir_chk     CHECK (direction IN ('SANTRI_TO_USTADZ','USTADZ_TO_SANTRI')),
    CONSTRAINT vr_rating_chk  CHECK (rating BETWEEN 1 AND 5),
    KEY vr_reviewee_idx (reviewee_id, revealed_at, hidden)
) ENGINE=InnoDB;
