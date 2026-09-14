#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
MQ — Import Quran reference data (Task 1.2 Bagian I).
Sumber:
  - Surah metadata      : quran.com API v4 /chapters?language=id (nama Indonesia = Kemenag)
  - Teks Uthmani/Imlaei : quran.com API v4 /quran/verses/{uthmani,imlaei} (tanzil-based)
  - Terjemahan Kemenag  : quran.com API v4 /quran/translations/33
  - Halaman mushaf      : quran.com API v4 /verses/by_page/{1..604}
  - Juz (tabel quran_juzs + kolom juz ayah): /juzs verse_mapping
  - Audio Murattal      : URL pattern everyayah.com (2 qari: Alafasy, Husary) — metadata only
Idempotent: upsert natural key. Cache di cache/ (resumable, rerun cepat).
Pemakaian: python import_quran.py [--step chapters|verses|trans|pages|juzs|audio|db|all]
           (default: all = fetch yang belum ada di cache + tulis DB)
"""
import io, json, os, sys, time, urllib.request, urllib.error

BASE = "https://api.quran.com/api/v4"
HERE = os.path.dirname(os.path.abspath(__file__))
CACHE = os.path.join(HERE, "cache")
SLEEP = 0.15
UA = {"User-Agent": "mq-backend-import/1.0 (Task 1.2)"}

RECITERS = [  # (code, folder everyayah)
    ("ALAFASY_128KBPS", "Alafasy_128kbps"),
    ("HUSARY_128KBPS", "Husary_128kbps"),
]

def api(path):
    req = urllib.request.Request(BASE + path, headers=UA)
    for attempt in range(5):
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                return json.loads(r.read().decode("utf-8"))
        except Exception as e:
            print(f"  retry {attempt+1} {path}: {e}")
            time.sleep(2 * (attempt + 1))
    raise SystemExit(f"GAGAL fetch {path}")

def cached(name, fetch):
    p = os.path.join(CACHE, name)
    if os.path.exists(p):
        with io.open(p, encoding="utf-8") as f:
            return json.load(f)
    data = fetch()
    os.makedirs(CACHE, exist_ok=True)
    with io.open(p, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False)
    return data

def paginate(path_fmt, key, per_page=50):
    """Fetch semua halaman; return list gabungan item."""
    out, page = [], 1
    while True:
        d = api(path_fmt.format(page=page, pp=per_page))
        items = d.get(key, [])
        out.extend(items)
        meta = d.get("meta", {}) or {}
        total = meta.get("total_count") or meta.get("total") or len(out)
        if len(out) >= total or not items:
            break
        page += 1
        time.sleep(SLEEP)
    return out

# ---------- steps ----------

def step_chapters():
    def fetch():
        d = api("/chapters?language=id")
        return d["chapters"]
    ch = cached("chapters.json", fetch)
    print(f"chapters: {len(ch)}")
    return ch

def step_verses():
    def fetch_uthmani():
        return paginate("/quran/verses/uthmani?page={page}&per_page={pp}", "verses")
    def fetch_imlaei():
        return paginate("/quran/verses/imlaei?page={page}&per_page={pp}", "verses")
    u = cached("verses_uthmani.json", fetch_uthmani)
    i = cached("verses_imlaei.json", fetch_imlaei)
    print(f"verses uthmani={len(u)} imlaei={len(i)}")
    assert len(u) == 6236 and len(i) == 6236, "jumlah ayat != 6236"
    return u, i

def step_trans():
    def fetch():
        return paginate("/quran/translations/33?page={page}&per_page={pp}", "translations")
    t = cached("translation_kemenag.json", fetch)
    print(f"translations: {len(t)}")
    assert len(t) == 6236, "terjemahan != 6236"
    return t

def step_pages():
    def fetch():
        mapping = {}
        for p in range(1, 605):
            d = api(f"/verses/by_page/{p}?fields=verse_key")
            for v in d.get("verses", []):
                mapping[v["verse_key"]] = p
            if p % 100 == 0:
                print(f"  ...page {p}/604 ({len(mapping)} ayat)")
            time.sleep(SLEEP)
        return mapping
    m = cached("pages.json", fetch)
    print(f"pages: {len(m)} ayat terpetakan")
    assert len(m) == 6236, "page mapping != 6236"
    return m

def parse_range(val):
    """'1-141' | '7' | '1,3,5' -> list int"""
    val = str(val).strip()
    out = []
    for part in val.split(","):
        part = part.strip()
        if "-" in part:
            a, b = part.split("-", 1)
            out.extend(range(int(a), int(b) + 1))
        elif part:
            out.append(int(part))
    return out

def step_juzs():
    def fetch():
        d = api("/juzs?language=id")
        seen, dedup = set(), []
        for j in d["juzs"]:
            n = j["juz_number"]
            if n not in seen:
                seen.add(n)
                dedup.append(j)   # API v4 kadang duplikat list juz
        return dedup
    j = cached("juzs.json", fetch)
    # cache lama (belum dedup) -> perbaiki on the fly
    if len(j) != 30:
        seen, dedup = set(), []
        for item in j:
            if item["juz_number"] not in seen:
                seen.add(item["juz_number"])
                dedup.append(item)
        j = dedup
    print(f"juzs: {len(j)}")
    assert len(j) == 30, f"juz != 30 ({len(j)})"
    return j

def step_words():
    """WBW per chapter: token char_type_name=='word' saja, re-number position 1..n."""
    out = {}
    for ch in range(1, 115):
        def fetch(ch=ch):
            items, page = [], 1
            while True:
                d = api(f"/verses/by_chapter/{ch}?words=true&per_page=50&page={page}"
                        "&fields=verse_key&word_fields=text_uthmani")
                vs = d.get("verses", [])
                if not vs:
                    break                 # endpoint ini meta=null -> lanjut sampai kosong
                items.extend(vs)
                page += 1
                time.sleep(SLEEP)
            return items
        out[str(ch)] = cached(f"words_{ch:03d}.json", fetch)
        if ch % 20 == 0:
            print(f"  ...words chapter {ch}/114")
    total_verses = sum(len(v) for v in out.values())
    print(f"words: 114 chapter cached, {total_verses} verses")
    assert total_verses == 6236, "verses words != 6236"
    return out

def derive_juz_of(surah, ayah, juz_starts):
    """juz_starts: list of (surah, ayah) awal juz 1..30 (sorted)."""
    lo, hi = 0, len(juz_starts) - 1
    res = 1
    for idx, (s, a) in enumerate(juz_starts):
        if (surah, ayah) >= (s, a):
            res = idx + 1
    return res

# ---------- DB ----------

def db_write(chapters, uthmani, imlaei, trans, pages, juzs, words=None):
    import pymysql
    env = {}
    with io.open(os.path.join(HERE, "..", "..", ".env"), encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                k, v = line.split("=", 1)
                env[k] = v
    url = env["DATABASE_URL"]  # mysql://user:pass@host:port/db
    assert url.startswith("mysql://"), url
    body = url[len("mysql://"):]
    cred, hostdb = body.rsplit("@", 1)
    user, pwd = cred.split(":", 1)
    hostdb = hostdb.split("/")
    hostport = hostdb[0]
    db = hostdb[1].split("?")[0]
    host, port = (hostport.split(":") + ["3306"])[:2]
    conn = pymysql.connect(host=host, port=int(port), user=user, password=pwd,
                           database=db, charset="utf8mb4", autocommit=False)
    cur = conn.cursor()
    try:
        # 1. surahs
        for c in chapters:
            rev = "MAKKAH" if c["revelation_place"] == "makkah" else "MADINAH"
            cur.execute(
                "INSERT INTO quran_surahs (id, name_arabic, name_latin, name_id, ayah_count, revelation) "
                "VALUES (%s,%s,%s,%s,%s,%s) AS new ON DUPLICATE KEY UPDATE "
                "name_arabic=new.name_arabic, name_latin=new.name_latin, name_id=new.name_id, "
                "ayah_count=new.ayah_count, revelation=new.revelation",
                (c["id"], c["name_arabic"], c["name_simple"],
                 c.get("translated_name", {}).get("name", c["name_simple"]),
                 c["verses_count"], rev))
        print("quran_surahs OK")

        # 2. juzs (start/end dari verse_mapping; value = string range 'a-b' atau 'a')
        juz_starts = []
        for j in juzs:
            vm = j.get("verse_mapping") or {}
            items = []
            for s, rng in vm.items():
                ay = parse_range(rng)
                if ay:
                    items.append((int(s), min(ay), max(ay)))
            items.sort()
            s0, a0, _ = items[0]
            s1, _, a1 = items[-1]
            juz_starts.append((s0, a0))
            cur.execute(
                "INSERT INTO quran_juzs (id, start_surah_id, start_ayah, end_surah_id, end_ayah) "
                "VALUES (%s,%s,%s,%s,%s) AS new ON DUPLICATE KEY UPDATE "
                "start_surah_id=new.start_surah_id, start_ayah=new.start_ayah, "
                "end_surah_id=new.end_surah_id, end_ayah=new.end_ayah",
                (j["juz_number"], s0, a0, s1, a1))
        print("quran_juzs OK (30)")

        # 3. ayahs (uthmani + imlaei + juz + page) — idempotent via ODKU natural key
        iml = {v["verse_key"]: v["text_imlaei"] for v in imlaei}
        for v in uthmani:
            key = v["verse_key"]
            s, a = (int(x) for x in key.split(":"))
            cur.execute(
                "INSERT INTO quran_ayahs (surah_id, ayah_number, text_uthmani, text_imlaei, juz, page) "
                "VALUES (%s,%s,%s,%s,%s,%s) AS new "
                "ON DUPLICATE KEY UPDATE text_uthmani=new.text_uthmani, text_imlaei=new.text_imlaei, "
                "juz=new.juz, page=new.page",
                (s, a, v["text_uthmani"], iml.get(key), derive_juz_of(s, a, juz_starts), pages[key]))
        print("quran_ayahs OK (6236)")

        # 4. translations (item ke-i = global verse id i+1, urut mushaf)
        #    INSERT...SELECT + ODKU: alias-row tidak bisa dipakai bersama SELECT,
        #    jadi pakai VALUES() (deprecated-warning di 8.0.20+, tetap jalan — importer one-off).
        for idx, t in enumerate(trans):
            v = uthmani[idx]
            s, a = (int(x) for x in v["verse_key"].split(":"))
            cur.execute(
                "INSERT INTO quran_translations (ayah_id, translator_code, text) "
                "SELECT qa.id, 'KEMENAG', %s FROM quran_ayahs qa WHERE qa.surah_id=%s AND qa.ayah_number=%s "
                "ON DUPLICATE KEY UPDATE text=VALUES(text)",
                (t["text"], s, a))
        print("quran_translations OK")

        # 5. audio URL (pattern everyayah)
        for code, folder in RECITERS:
            for v in uthmani:
                s, a = (int(x) for x in v["verse_key"].split(":"))
                url = f"https://everyayah.com/data/{folder}/{s:03d}{a:03d}.mp3"
                cur.execute(
                    "INSERT INTO quran_audio_files (ayah_id, reciter_code, audio_url) "
                    "SELECT qa.id, %s, %s FROM quran_ayahs qa WHERE qa.surah_id=%s AND qa.ayah_number=%s "
                    "ON DUPLICATE KEY UPDATE audio_url=VALUES(audio_url)",
                    (code, url, s, a))
        print(f"quran_audio_files OK ({len(RECITERS)} reciter x 6236)")

        # 6. quran_words (WBW) — token 'word' saja, position re-number; transliteration + EN
        n_words = 0
        for ch_str, verses in words.items():
            for v in verses:
                s, a = (int(x) for x in v["verse_key"].split(":"))
                pos = 0
                for w in v.get("words", []):
                    if w.get("char_type_name") != "word":
                        continue
                    pos += 1
                    tr = w.get("translation") or {}
                    tr = tr.get("text") if isinstance(tr, dict) else tr
                    li = w.get("transliteration") or {}
                    li = li.get("text") if isinstance(li, dict) else li
                    cur.execute(
                        "INSERT INTO quran_words (ayah_id, position, text_uthmani, transliteration, text_en) "
                        "SELECT qa.id, %s, %s, %s, %s FROM quran_ayahs qa WHERE qa.surah_id=%s AND qa.ayah_number=%s "
                        "ON DUPLICATE KEY UPDATE text_uthmani=VALUES(text_uthmani), transliteration=VALUES(transliteration), text_en=VALUES(text_en)",
                        (pos, w.get("text_uthmani") or w.get("text"), li, tr, s, a))
                    n_words += 1
        print(f"quran_words OK ({n_words} kata)")

        conn.commit()
    except Exception:
        conn.rollback()
        raise
    finally:
        conn.close()

    # verify
    conn = pymysql.connect(host=host, port=int(port), user=user, password=pwd,
                           database=db, charset="utf8mb4")
    cur = conn.cursor()
    for tbl in ["quran_surahs", "quran_juzs", "quran_ayahs", "quran_translations", "quran_audio_files"]:
        cur.execute(f"SELECT COUNT(*) FROM {tbl}")
        print(f"{tbl}: {cur.fetchone()[0]}")
    cur.execute("SELECT text_uthmani FROM quran_ayahs WHERE surah_id=1 AND ayah_number=1")
    print("sample 1:1:", cur.fetchone()[0][:60], "...")
    if words:
        cur.execute("SELECT COUNT(*) FROM quran_words")
        print("quran_words:", cur.fetchone()[0])
        cur.execute("SELECT qw.position, qw.text_uthmani, qw.transliteration, qw.text_en FROM quran_words qw "
                    "JOIN quran_ayahs qa ON qa.id=qw.ayah_id WHERE qa.surah_id=1 AND qa.ayah_number=1 ORDER BY qw.position")
        for row in cur.fetchall():
            print("  wbw:", row)
    conn.close()

def main():
    step = sys.argv[sys.argv.index("--step") + 1] if "--step" in sys.argv else "all"
    ch = step_chapters()
    u, i = step_verses()
    t = step_trans()
    p = step_pages()
    j = step_juzs()
    w = step_words()
    if step in ("all", "db"):
        db_write(ch, u, i, t, p, j, w)
    print("IMPORT SELESAI")

if __name__ == "__main__":
    main()
