"""Turn a round file into one JSON document per card, ready to write to the sign-off sheet.

Usage: python3 tools/signoff/build_round.py tools/signoff/rounds/r01.json OUT_DIR

Figures with "html_file" get the mockup inlined as "html". Writes OUT_DIR/cards/<id>.json and
OUT_DIR/meta/board.json, then prints the file list for an ArtifactData batch.
"""
import json
import os
import sys
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))


def main(src, out):
    with open(src, encoding="utf-8") as f:
        rnd = json.load(f)
    now = datetime.now(timezone.utc).isoformat(timespec="seconds")
    os.makedirs(os.path.join(out, "cards"), exist_ok=True)
    os.makedirs(os.path.join(out, "meta"), exist_ok=True)
    for cid, card in rnd["cards"].items():
        for fig in card.get("figures", []):
            path = fig.pop("html_file", None)
            if path:
                with open(os.path.join(HERE, path), encoding="utf-8") as m:
                    fig["html"] = m.read()
        card.setdefault("archived", False)
        card.setdefault("created_at", now)
        dest = os.path.join(out, "cards", cid + ".json")
        with open(dest, "w", encoding="utf-8") as f:
            json.dump(card, f, ensure_ascii=False)
        size = os.path.getsize(dest)
        if size > 250_000:
            sys.exit(f"{cid}: {size} bytes, over the 256 KiB document limit")
        print("cards", cid, dest)
    meta = dict(rnd.get("meta", {}), updated_at=now)
    dest = os.path.join(out, "meta", "board.json")
    with open(dest, "w", encoding="utf-8") as f:
        json.dump(meta, f, ensure_ascii=False)
    print("meta", "board", dest)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
