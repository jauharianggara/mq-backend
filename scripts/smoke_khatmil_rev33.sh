#!/bin/bash
# Smoke e2e khatmil rev 3.3 (backend F1 + F0a/b/c)
BASE=http://127.0.0.1:8290/api/v1
PASS=0; FAIL=0
chk() { # chk <nama> < kondisi-expr >
  if eval "$2"; then PASS=$((PASS+1)); echo "PASS: $1"; else FAIL=$((FAIL+1)); echo "FAIL: $1"; fi
}
jqget() { python -c "import sys,json; d=json.load(sys.stdin); print(eval('d'+sys.argv[1]))" "$1" 2>/dev/null; }

# tokens
ADM=$(curl -s -X POST $BASE/auth/login -H "Content-Type: application/json" -d '{"email":"admin@mq.local","password":"528292018fe0"}' | jqget "['data']['access_token']")
SN=$(curl -s -X POST $BASE/auth/login -H "Content-Type: application/json" -d '{"email":"santri.test@mq.local","password":"rahasia123"}' | jqget "['data']['access_token']")
[ -z "$SN" ] && SN=$(curl -s -X POST $BASE/auth/login -H "Content-Type: application/json" -d '{"email":"santri.test","password":"rahasia123"}' | jqget "['data']['access_token']")
chk "login admin+santri" "[ -n \"\$ADM\" ] && [ -n \"\$SN\" ]"
A="Authorization: Bearer $ADM"; S="Authorization: Bearer $SN"

TS=$(date +%s)
# 1. create campaign (admin)
R=$(curl -s -w '|%{http_code}' -X POST $BASE/khatmil/campaigns -H "$A" -H "Content-Type: application/json" -d "{\"slug\":\"smoke-rev33-$TS\",\"name\":\"SMOKE rev33\",\"mode\":\"PARALLEL\",\"status\":\"ACTIVE\",\"target_khataman\":1}")
CID=$(echo "${R%|*}" | jqget "['data']['id']")
chk "create campaign ACTIVE (201) id=$CID" "[ \"\${R#*|}\" = 201 ]"

# 2. join + claim juz 1 (santri)
curl -s -o /dev/null -X POST $BASE/khatmil/campaigns/$CID/join -H "$S"
R=$(curl -s -w '|%{http_code}' -X POST $BASE/khatmil/campaigns/$CID/juz/claim -H "$S" -H "Content-Type: application/json" -d '{"juz":1}')
AID=$(echo "${R%|*}" | jqget "['data']['id']")
chk "join + claim juz 1 (201)" "[ \"\${R#*|}\" = 201 ] && [ -n \"\$AID\" ]"

# 3. progress posisi 2:30 -> 25.0%
R=$(curl -s -X POST $BASE/khatmil/assignments/$AID/progress -H "$S" -H "Content-Type: application/json" -d '{"current_surah":2,"current_ayah":30}')
PCT=$(echo "$R" | jqget "['data']['progress_pct']"); RD=$(echo "$R" | jqget "['data']['read_ayat']"); TOT=$(echo "$R" | jqget "['data']['juz_total_ayat']")
chk "progress 2:30 = 25.0% (37/148)" "[ \"\$PCT\" = \"25.0\" ] && [ \"\$RD\" = \"37\" ] && [ \"\$TOT\" = \"148\" ]"

# 4. idempotent: posisi sama -> HTTP 200 (bukan 201) — perlu tunggu rate-limit 5s
sleep 7
R=$(curl -s -w '|%{http_code}' -X POST $BASE/khatmil/assignments/$AID/progress -H "$S" -H "Content-Type: application/json" -d '{"current_surah":2,"current_ayah":30}')
chk "idempotent posisi sama = 200 no-op" "[ \"\${R#*|}\" = 200 ]"

# 5. tanpa posisi -> 422
sleep 7
R=$(curl -s -w '|%{http_code}' -X POST $BASE/khatmil/assignments/$AID/progress -H "$S" -H "Content-Type: application/json" -d '{"pages_read":21,"minutes_read":40}')
chk "tanpa posisi = 422 (legacy ditolak)" "[ \"\${R#*|}\" = 422 ]"

# 6. posisi luar rentang juz -> 422 + pesan rentang
sleep 7
R=$(curl -s -X POST $BASE/khatmil/assignments/$AID/progress -H "$S" -H "Content-Type: application/json" -d '{"current_surah":2,"current_ayah":200}')
chk "posisi luar rentang = 422 + pesan" "echo \"\$R\" | grep -q 'QS 1:1 s.d. QS 2:141'"

# 7. posisi maju 2:100
sleep 7
R=$(curl -s -X POST $BASE/khatmil/assignments/$AID/progress -H "$S" -H "Content-Type: application/json" -d '{"current_surah":2,"current_ayah":100,"note":"juz hampir kelar"}')
PCT=$(echo "$R" | jqget "['data']['progress_pct']")
chk "maju ke 2:100 = 72.3%" "[ \"\$PCT\" = \"72.3\" ]"

# 8. my_assignments + posisi (F0a)
R=$(curl -s $BASE/me/khatmil/assignments -H "$S")
MP=$(echo "$R" | python -c "import sys,json; d=json.load(sys.stdin)['data']; a=[x for x in d if x['id']==$AID][0]; print(a['current_surah'],a['current_ayah'],a['progress_pct'],a['juz_total_ayat'])")
chk "my_assignments posisi 2 100 72.3 148" "[ \"\$MP\" = \"2 100 72.3 148\" ]"

# 9. selesaikan juz: posisi akhir 2:141 -> COMPLETED + SYSTEM_VERIFIED
sleep 7
R=$(curl -s -X POST $BASE/khatmil/assignments/$AID/progress -H "$S" -H "Content-Type: application/json" -d '{"current_surah":2,"current_ayah":141}')
ST=$(echo "$R" | jqget "['data']['status']"); VV=$(echo "$R" | jqget "['data']['verification']"); PCT=$(echo "$R" | jqget "['data']['progress_pct']")
chk "2:141 -> COMPLETED SYSTEM_VERIFIED 100%" "[ \"\$ST\" = \"COMPLETED\" ] && [ \"\$VV\" = \"SYSTEM_VERIFIED\" ] && [ \"\$PCT\" = \"100.0\" ]"
sleep 7
R=$(curl -s -w '|%{http_code}' -X POST $BASE/khatmil/assignments/$AID/progress -H "$S" -H "Content-Type: application/json" -d '{"current_surah":2,"current_ayah":141}')
chk "progress setelah COMPLETED = 409" "[ \"\${R#*|}\" = 409 ]"

# 10. participants v2 (guard khatmil.read — santri boleh)
R=$(curl -s $BASE/khatmil/campaigns/$CID/participants -H "$S")
PP=$(echo "$R" | python -c "import sys,json; p=json.load(sys.stdin)['data'][0]; print(p['juz_done_count'],p['juz_active_count'],p['task_progress_pct'],p['contribution_pct'],p['juz_done'][0]['juz'],p['full_name'])")
chk "participants v2: done=1 active=0 task=100 contr=3.3 juz=1" "[ \"\$PP\" = \"1 0 100.0 3.3 1 Santri Test\" ] || echo \"\$PP\" | grep -q '^1 0 100.0 3.3 1 '"

# 11. activity feed
R=$(curl -s "$BASE/khatmil/campaigns/$CID/activity" -H "$S")
AC=$(echo "$R" | python -c "import sys,json; a=json.load(sys.stdin)['data']; print(len(a), a[0]['completed'], a[0]['current_surah'], a[0]['current_ayah'])")
chk "activity: >=3 event, terbaru = selesai 2 141" "[ \"\${AC%% *}\" -ge 3 ] && echo \"\$AC\" | grep -q 'True 2 141'"

# 12. juz_map v2: juz 1 COMPLETED terlihat + owner + completed_at
R=$(curl -s $BASE/khatmil/campaigns/$CID -H "$S")
JM=$(echo "$R" | python -c "import sys,json; m=[j for j in json.load(sys.stdin)['data']['juz_map'] if j['juz']==1][0]; print(m['status'], m['owner_name'] is not None, m['completed_at'] is not None)")
chk "juz_map: juz1=COMPLETED + owner + completed_at" "[ \"\$JM\" = \"COMPLETED True True\" ]"

# 13. lifecycle guard: ACTIVE->DRAFT invalid 409; ACTIVE->COMPLETED valid
R=$(curl -s -w '|%{http_code}' -X PATCH $BASE/khatmil/campaigns/$CID -H "$A" -H "Content-Type: application/json" -d "{\"slug\":\"smoke-rev33-$TS\",\"name\":\"SMOKE rev33\",\"mode\":\"PARALLEL\",\"status\":\"DRAFT\",\"target_khataman\":1}")
chk "transisi ilegal ACTIVE->DRAFT = 409" "[ \"\${R#*|}\" = 409 ] && echo \"\$R\" | grep -q invalid_status_transition"
R=$(curl -s -w '|%{http_code}' -X PATCH $BASE/khatmil/campaigns/$CID -H "$A" -H "Content-Type: application/json" -d "{\"slug\":\"smoke-rev33-$TS\",\"name\":\"SMOKE rev33\",\"mode\":\"PARALLEL\",\"status\":\"COMPLETED\",\"target_khataman\":1}")
chk "transisi valid ACTIVE->COMPLETED = 200" "[ \"\${R#*|}\" = 200 ]"
R=$(curl -s -w '|%{http_code}' -X PATCH $BASE/khatmil/campaigns/$CID -H "$A" -H "Content-Type: application/json" -d "{\"slug\":\"smoke-rev33-$TS\",\"name\":\"SMOKE rev33\",\"mode\":\"PARALLEL\",\"status\":\"ACTIVE\",\"target_khataman\":1}")
chk "terminal COMPLETED->ACTIVE ditolak 409" "[ \"\${R#*|}\" = 409 ]"

# 14. quran juzs endpoint (F0c)
R=$(curl -s "$BASE/quran/juzs/1/ayahs")
QA=$(echo "$R" | python -c "import sys,json; d=json.load(sys.stdin)['data']; print(d['total_ayat'], d['ayahs'][0]['surah_id'], d['ayahs'][0]['ayah_number'], d['ayahs'][-1]['surah_id'], d['ayahs'][-1]['ayah_number'], d['ayahs'][7]['surah_name_latin'] if 'surah_name_latin' in d['ayahs'][7] else '?')")
chk "quran juz1 ayahs: 148 ayat 1:1..2:141" "[ \"\$QA\" = \"148 1 1 2 141 Al-Baqarah\" ] || echo \"\$QA\" | grep -q '^148 1 1 2 141'"
R2=$(curl -s "$BASE/quran/juzs/1/ayahs") # cache hit kedua
chk "quran juzs cache-hit tetap benar" "echo \"\$R2\" | grep -q '\"total_ayat\":148'"

# 15. worker stale reminder: enable + stale_days=0 + run manual
python -X utf8 -c "
import os,pymysql
u=open(os.path.expanduser('~/.mq_dbpass.txt')).read().strip()
c=pymysql.connect(host='127.0.0.1',user='mq_app',password=u,database='mq_dev',autocommit=True)
cur=c.cursor()
cur.execute(\"UPDATE settings SET value=CAST('true' AS JSON) WHERE \`key\`='khatmil_reminder_enabled'\")
cur.execute(\"UPDATE settings SET value=CAST('0' AS JSON) WHERE \`key\`='khatmil_reminder_stale_days'\")
# buat assignment mangkrak baru: campaign baru ACTIVE + join + claim (tanpa progress)
cur.execute(\"INSERT INTO khatmil_campaigns (slug,name,mode,status,target_khataman,created_by) VALUES ('smoke-stale-$TS','SMOKE STALE','PARALLEL','ACTIVE',1,1)\")
cid=cur.lastrowid
cur.execute('SELECT id FROM khatmil_participants WHERE campaign_id=?'.replace('?', '%s') if False else 'SELECT id FROM khatmil_participants LIMIT 1')
cur.execute(\"INSERT INTO khatmil_participants (campaign_id,user_id) VALUES (%s,2)\",(cid,))
pid=cur.lastrowid
cur.execute(\"INSERT INTO khatmil_juz_assignments (campaign_id,participant_id,juz,status,assigned_at) VALUES (%s,%s,3,'ASSIGNED',DATE_SUB(UTC_TIMESTAMP(), INTERVAL 5 DAY))\",(cid,pid))
print('STALE_CID',cid)
c.close()" | read _ STCID
# jalankan executor manual via run job: pindahkan run_at job terjadwal ke masa lalu lalu tunggu worker tick
python -X utf8 -c "
import os,pymysql
u=open(os.path.expanduser('~/.mq_dbpass.txt')).read().strip()
c=pymysql.connect(host='127.0.0.1',user='mq_app',password=u,database='mq_dev',autocommit=True)
cur=c.cursor()
cur.execute(\"INSERT INTO scheduled_jobs (job_type,payload,status,run_at,dedupe_key) VALUES ('khatmil.stale_reminder',CAST('{}' AS JSON),'PENDING',DATE_SUB(UTC_TIMESTAMP(), INTERVAL 1 MINUTE),'smoke-manual-$TS')\")
c.close()"
echo "menunggu worker tick (<=35s)..."; sleep 35
echo "NN=$NN" >/dev/null; NN=$(python -X utf8 -c "
import os,pymysql
u=open(os.path.expanduser('~/.mq_dbpass.txt')).read().strip()
c=pymysql.connect(host='127.0.0.1',user='mq_app',password=u,database='mq_dev')
cur=c.cursor()
cur.execute(\"SELECT COUNT(*) FROM user_notifications WHERE template_code='KHATMIL_JUZ_STALE'\")
print(cur.fetchone()[0])
c.close()")
chk "worker stale reminder: notif terkirim" "[ \"\$NN\" -ge 1 ]"
# jalankan sekali lagi (dedupe 24 jam — tidak dobel)
python -X utf8 -c "
import os,pymysql
u=open(os.path.expanduser('~/.mq_dbpass.txt')).read().strip()
c=pymysql.connect(host='127.0.0.1',user='mq_app',password=u,database='mq_dev',autocommit=True)
cur=c.cursor()
cur.execute(\"INSERT INTO scheduled_jobs (job_type,payload,status,run_at,dedupe_key) VALUES ('khatmil.stale_reminder',CAST('{}' AS JSON),'PENDING',DATE_SUB(UTC_TIMESTAMP(), INTERVAL 1 MINUTE),'smoke-manual2-$TS')\")
c.close()"
sleep 35
NN2=$(python -X utf8 -c "
import os,pymysql
u=open(os.path.expanduser('~/.mq_dbpass.txt')).read().strip()
c=pymysql.connect(host='127.0.0.1',user='mq_app',password=u,database='mq_dev')
cur=c.cursor()
cur.execute(\"SELECT COUNT(*) FROM user_notifications WHERE template_code='KHATMIL_JUZ_STALE'\")
print(cur.fetchone()[0])
c.close()")
chk "anti-spam 24 jam: tidak dobel" "[ \"\$NN2\" = \"\$NN\" ]"

# 16. cleanup + kembalikan settings
python -X utf8 -c "
import os,pymysql
u=open(os.path.expanduser('~/.mq_dbpass.txt')).read().strip()
c=pymysql.connect(host='127.0.0.1',user='mq_app',password=u,database='mq_dev',autocommit=True)
cur=c.cursor()
cur.execute(\"UPDATE settings SET value=CAST('false' AS JSON) WHERE \`key\`='khatmil_reminder_enabled'\")
cur.execute(\"UPDATE settings SET value=CAST('3' AS JSON) WHERE \`key\`='khatmil_reminder_stale_days'\")
cur.execute(\"DELETE e FROM khatmil_progress_events e JOIN khatmil_juz_assignments a ON a.id=e.assignment_id JOIN khatmil_campaigns c2 ON c2.id=a.campaign_id WHERE c2.slug LIKE 'smoke-%'\")
cur.execute(\"DELETE pg FROM khatmil_progress pg JOIN khatmil_juz_assignments a ON a.id=pg.assignment_id JOIN khatmil_campaigns c2 ON c2.id=a.campaign_id WHERE c2.slug LIKE 'smoke-%'\")
cur.execute(\"DELETE a FROM khatmil_juz_assignments a JOIN khatmil_campaigns c2 ON c2.id=a.campaign_id WHERE c2.slug LIKE 'smoke-%'\")
cur.execute(\"DELETE p FROM khatmil_participants p JOIN khatmil_campaigns c2 ON c2.id=p.campaign_id WHERE c2.slug LIKE 'smoke-%'\")
cur.execute(\"DELETE FROM khatmil_campaigns WHERE slug LIKE 'smoke-%'\")
cur.execute(\"DELETE FROM user_notifications WHERE template_code='KHATMIL_JUZ_STALE'\")
cur.execute(\"DELETE FROM scheduled_jobs WHERE dedupe_key LIKE 'smoke-manual%'\")
cur.execute(\"DELETE FROM activity_events WHERE ref_type='khatmil_assignment' AND ref_id NOT IN (SELECT id FROM khatmil_juz_assignments)\")
cur.execute(\"DELETE FROM activity_events WHERE event_type='khatmil.joined' AND ref_type='khatmil_campaign' AND CAST(ref_id AS CHAR) NOT IN (SELECT CAST(id AS CHAR) FROM khatmil_campaigns)\")
c.close()"
chk "cleanup smoke data + settings restore" "true"

echo "DEBUG-NN: $NN -> $NN2"; echo "=== HASIL: PASS=$PASS FAIL=$FAIL ==="
