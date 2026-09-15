# -*- coding: utf-8 -*-
# Seed khatmil demo (plan seed-people Fase 3): campaign + 70 santri @mq-demo.id
# Sebaran: 15 selesai 1-3 juz (SYSTEM_VERIFIED, posisi akhir), 25 in-progress
# posisi merata 5-95%, 30 join saja. Tanggal laporan mundur sampai 30 hari.
# Idempotent: skip bila campaign sudah ada. Run: python -X utf8 scripts/seed_khatmil_demo.py
import os, sys, random
import pymysql

random.seed(42)
u = open(os.path.expanduser('~/.mq_dbpass.txt')).read().strip()
conn = pymysql.connect(host='127.0.0.1', user='mq_app', password=u, database='mq_dev', autocommit=False)
cur = conn.cursor()

SLUG = 'khataman-santri-mq-1'
cur.execute('SELECT id, status FROM khatmil_campaigns WHERE slug=%s', (SLUG,))
row = cur.fetchone()
if row:
    pass  # campaign sudah ada — lanjutkan sebaran (idempotent per-bagian)
cid = row[0]
cur.execute('DELETE pg FROM khatmil_progress pg JOIN khatmil_juz_assignments a ON a.id=pg.assignment_id WHERE a.campaign_id=%s', (cid,))
cur.execute('DELETE FROM khatmil_juz_assignments WHERE campaign_id=%s', (cid,))
conn.commit()

# juz bounds + total ayat
cur.execute('SELECT id, start_surah_id, start_ayah, end_surah_id, end_ayah FROM quran_juzs ORDER BY id')
juzs = {r[0]: r for r in cur.fetchall()}
def juz_total(j):
    cur.execute('SELECT COUNT(*) FROM quran_ayahs WHERE juz=%s', (j,))
    return cur.fetchone()[0]
def pos_at_offset(j, offset):  # offset 1-based
    cur.execute('SELECT surah_id, ayah_number FROM quran_ayahs WHERE juz=%s ORDER BY surah_id, ayah_number LIMIT 1 OFFSET %s', (j, offset-1))
    return cur.fetchone()

# 70 santri demo
cur.execute("""
    SELECT u.id, p.id FROM users u
    JOIN khatmil_participants p ON p.user_id = u.id
    JOIN khatmil_campaigns c ON c.id = p.campaign_id
    WHERE c.slug = %s ORDER BY u.id""", (SLUG,))
parts = cur.fetchall()
print('peserta:', len(parts))

def claim(cur, cid, part_id, juz, days_ago):
    cur.execute('SELECT id FROM khatmil_juz_assignments WHERE campaign_id=%s AND participant_id=%s AND juz=%s', (cid, part_id, juz))
    ex = cur.fetchone()
    if ex:
        return ex[0]
    cur.execute("INSERT INTO khatmil_juz_assignments (campaign_id, participant_id, juz, status, assigned_at) VALUES (%s,%s,%s,'ASSIGNED',DATE_SUB(UTC_TIMESTAMP(), INTERVAL %s DAY))",
                (cid, part_id, juz, days_ago))
    return cur.lastrowid

def report(cur, uid, aid, juz, s, a, pages, minutes, days_ago):
    cur.execute("INSERT INTO khatmil_progress_events (assignment_id, user_id, pages_read, minutes_read, current_surah_id, current_ayah, recorded_at) VALUES (%s,%s,%s,%s,%s,%s,DATE_SUB(UTC_TIMESTAMP(), INTERVAL %s DAY))",
                (aid, uid, pages, minutes, s, a, days_ago))
    completed = (s, a) == (juzs[juz][3], juzs[juz][4])
    verification = 'SYSTEM_VERIFIED' if completed else 'SELF_REPORTED'
    vset = 'NOW()' if completed else 'NULL'
    cur.execute(f"""INSERT INTO khatmil_progress (assignment_id, pages_read, minutes_read, verification, verified_at, current_surah_id, current_ayah)
        VALUES (%s,%s,%s,%s,{vset},%s,%s) AS new
        ON DUPLICATE KEY UPDATE pages_read=GREATEST(new.pages_read,khatmil_progress.pages_read),
        minutes_read=GREATEST(new.minutes_read,khatmil_progress.minutes_read),
        verification=new.verification, verified_at=COALESCE({vset},khatmil_progress.verified_at),
        current_surah_id=new.current_surah_id, current_ayah=new.current_ayah""",
        (aid, pages, minutes, verification, s, a))
    cur.execute('UPDATE khatmil_juz_assignments SET status=%s WHERE id=%s', ('COMPLETED' if completed else 'IN_PROGRESS', aid))

n_done_users = n_prog_users = 0
for i, (uid, part_id) in enumerate(parts):
    rn = i + 1
    if rn <= 25:
        # IN_PROGRESS: juz = rn (1-25), posisi 5%-95% merata
        juz = rn
        total = juz_total(juz)
        frac = 0.05 + (rn % 19) * 0.05  # 5%..95%
        offset = max(1, min(total, int(total * frac)))
        s, a = pos_at_offset(juz, offset)
        aid = claim(cur, cid, part_id, juz, days_ago=10)
        for day in (10, 7, 4, 1):
            report(cur, uid, aid, juz, s, a, random.randint(3, 15), random.randint(15, 45), day)
        n_prog_users += 1
    elif rn <= 40:
        # selesai 1 juz: juz 26-30 bergilir (completed melepas marker -> boleh dipakai ulang)
        juz = 26 + ((rn - 26) % 5)
        total = juz_total(juz)
        es, ea = juzs[juz][3], juzs[juz][4]
        aid = claim(cur, cid, part_id, juz, days_ago=15)
        report(cur, uid, aid, juz, es, ea, 20, 35, 5)
        n_done_users += 1
    # rn > 40: join saja
conn.commit()
print(f'seed khatmil demo OK: {len(parts)} peserta | {n_prog_users} in-progress, {n_done_users} selesai juz')
conn.close()
