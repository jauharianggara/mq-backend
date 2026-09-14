#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Generate docs/erd.dbml (dbdiagram.io) dari skema MySQL aktual (mq_dev)."""
import io, os, re, sys
import pymysql

HERE = os.path.dirname(os.path.abspath(__file__))

def db():
    env = {}
    with io.open(os.path.join(HERE, "..", ".env"), encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                k, v = line.split("=", 1)
                env[k] = v
    url = env["DATABASE_URL"]
    body = url[len("mysql://"):]
    cred, hostdb = body.rsplit("@", 1)
    user, pwd = cred.split(":", 1)
    hp, dbn = hostdb.split("/")
    host, port = (hp.split(":") + ["3306"])[:2]
    return pymysql.connect(host=host, port=int(port), user=user, password=pwd,
                           database=dbn.split("?")[0], charset="utf8mb4")

def main():
    conn = db()
    cur = conn.cursor()
    cur.execute("""SELECT table_name, column_name, column_type, is_nullable, column_key,
                          extra, column_comment
                   FROM information_schema.columns
                   WHERE table_schema = DATABASE()
                   ORDER BY table_name, ordinal_position""")
    tables = {}
    for (t, c, ty, null, key, extra, comment) in cur.fetchall():
        if t.startswith("_mq_"):
            continue
        tables.setdefault(t, []).append((c, ty, null, key, extra, comment))

    cur.execute("""SELECT kcu.table_name, kcu.column_name, kcu.referenced_table_name,
                          kcu.referenced_column_name, rc.delete_rule
                   FROM information_schema.key_column_usage kcu
                   JOIN information_schema.referential_constraints rc
                     ON rc.constraint_name = kcu.constraint_name
                    AND rc.constraint_schema = kcu.constraint_schema
                   WHERE kcu.table_schema = DATABASE()
                     AND kcu.referenced_table_name IS NOT NULL""")
    fks = cur.fetchall()
    conn.close()

    out = io.StringIO()
    out.write("// MQ Digital Platform — ERD (generate otomatis dari mq_dev, Task 1.3)\n")
    out.write("// Render: https://dbdiagram.io/d — paste file ini.\n\n")
    for t in sorted(tables):
        cols = tables[t]
        out.write(f'Table "{t}" {{\n')
        for (c, ty, null, key, extra, comment) in cols:
            note = f" [note: {comment.replace(chr(34), '')}]" if comment else ""
            pk = " [pk]" if key == "PRI" else ""
            uq = " [unique]" if key == "UNI" else ""
            nn = "" if null == "YES" or key == "PRI" else " [not null]"
            if extra and "auto_increment" in extra:
                nn += " [increment]"
            out.write(f"  {c} {ty}{pk}{uq}{nn}{note}\n")
        out.write("}\n\n")
    seen = set()
    for (t, c, rt, rc, dr) in sorted(fks):
        if (t, c, rt, rc) in seen:
            continue
        seen.add((t, c, rt, rc))
        sym = "-" if dr == "CASCADE" else "<"
        out.write(f'Ref: "{t}"."{c}" {sym}> "{rt}"."{rc}"\n')

    with io.open(os.path.join(HERE, "..", "docs", "erd.dbml"), "w", encoding="utf-8") as f:
        f.write(out.getvalue())
    print(f"docs/erd.dbml: {len(tables)} tabel, {len(seen)} relasi FK")

if __name__ == "__main__":
    main()
