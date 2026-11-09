"""Builds catalog/<version>.gob of the Star Citizen export plugin from a local install.

    python -I dev/catalog.py G:/RSI/StarCitizen/LIVE [G:/RSI/StarCitizen/PTU ...]

Reads build_manifest.id (version), Data.p4k (Data/Libs/Config/defaultProfile.xml, CryXmlB,
zstd) and data/Localization/english/global.ini (names), only reading. Prints rename candidates
against the newest older catalog: actions gone from a group while one with the same name came.
Needs Python 3.14 (compression.zstd). Run by the maintainer on request; not shipped.
"""

import json
import struct
import sys
import tomllib
import xml.etree.ElementTree as ET
from datetime import date
from pathlib import Path

from compression import zstd

PLUGIN = Path(__file__).resolve().parent.parent
AXES = {"x", "y", "z", "rotx", "roty", "rotz", "slider1", "slider2"}
THUMBS = {"thumblx", "thumbly", "thumbrx", "thumbry"}


def p4k_file(p4k: Path, wanted: str) -> bytes:
    """One file of a p4k archive (zip64 with zstd entries and extra fields zipfile rejects)."""
    wanted = wanted.lower().replace("/", "\\")
    with open(p4k, "rb") as f:
        f.seek(0, 2)
        size = f.tell()
        f.seek(max(0, size - 65536 - 22))
        tail = f.read()
        loc = struct.unpack_from("<IIQI", tail, tail.rfind(b"PK\x06\x07"))
        f.seek(loc[2])
        e64 = struct.unpack("<IQHHIIQQQQ", f.read(56))
        count, cd_size, cd_off = e64[7], e64[8], e64[9]
        f.seek(cd_off)
        cd = f.read(cd_size)
        p = 0
        for _ in range(count):
            h = struct.unpack_from("<IHHHHHHIIIHHHHHII", cd, p)
            method, csz, usz, nl, el, cl, off = h[4], h[8], h[9], h[10], h[11], h[12], h[16]
            name = cd[p + 46 : p + 46 + nl].decode("utf-8", "replace")
            if name.lower() == wanted:
                extra = cd[p + 46 + nl : p + 46 + nl + el]
                q = 0
                while q + 4 <= len(extra):
                    tp, ln = struct.unpack_from("<HH", extra, q)
                    if tp == 1:
                        vals = struct.unpack_from("<" + "Q" * (ln // 8), extra, q + 4)
                        k = 0
                        if usz == 0xFFFFFFFF:
                            k += 1
                        if csz == 0xFFFFFFFF:
                            csz = vals[k]
                            k += 1
                        if off == 0xFFFFFFFF:
                            off = vals[k]
                    q += 4 + ln
                f.seek(off)
                lnl, lel = struct.unpack_from("<HH", f.read(30), 26)
                f.seek(off + 30 + lnl + lel)
                data = f.read(csz)
                return zstd.decompress(data) if method == 100 else data
            p += 46 + nl + el + cl
    raise SystemExit(f"{wanted} not found in {p4k}")


def cryxml(b: bytes) -> ET.Element:
    if b[:8] != b"CryXmlB\0":
        return ET.fromstring(b)
    (_, npos, ncnt, apos, acnt, cpos, ccnt, spos, _) = struct.unpack_from("<9I", b, 8)

    def s(o):
        e = b.index(b"\0", spos + o)
        return b[spos + o : e].decode("utf-8", "replace")

    nodes = [struct.unpack_from("<IIHHiiii", b, npos + i * 28) for i in range(ncnt)]
    attrs = [struct.unpack_from("<II", b, apos + i * 8) for i in range(acnt)]
    kids = [struct.unpack_from("<i", b, cpos + i * 4)[0] for i in range(ccnt)]

    def build(i):
        tag, _, na, nc, _, fa, fc, _ = nodes[i]
        el = ET.Element(s(tag), {s(attrs[fa + k][0]): s(attrs[fa + k][1]) for k in range(na)})
        for k in range(nc):
            el.append(build(kids[fc + k]))
        return el

    return build(0)


def localization(channel: Path) -> dict:
    path = channel / "data" / "Localization" / "english" / "global.ini"
    if path.is_file():
        text = path.read_bytes()
    else:
        text = p4k_file(channel / "Data.p4k", "Data/Localization/english/global.ini")
    out = {}
    for line in text.decode("utf-8-sig", "replace").splitlines():
        k, sep, v = line.partition("=")
        if sep:
            out.setdefault(k.split(",")[0].strip(), v.strip())
    return out


def is_axis(a: ET.Element) -> bool:
    js = a.get("joystick", "").strip()
    gp = a.get("gamepad", "").strip().split("+")[-1]
    return js in AXES or gp in THUMBS or a.get("mouse", "").startswith("maxis")


def actions(channel: Path):
    profile = cryxml(p4k_file(channel / "Data.p4k", "Data/Libs/Config/defaultProfile.xml"))
    loc = localization(channel)

    def name(label):
        return loc.get(label.lstrip("@"), label.lstrip("@")) if label else ""

    for m in profile.iter("actionmap"):
        group = name(m.get("UILabel", "")) or m.get("name")
        for a in m.findall("action"):
            label = a.get("UILabel")
            if not label:
                continue
            if "mouse" in a.attrib and not any(
                k in a.attrib for k in ("keyboard", "joystick", "gamepad")
            ):
                continue
            default = []
            for attr, prefix in (("joystick", "js1_"), ("keyboard", "kb1_")):
                v = a.get(attr, "").strip()
                if v:
                    default.append(prefix + v)
            yield {
                "id": f"{m.get('name')}/{a.get('name')}",
                "category": group,
                "name": name(label),
                "kind": "axis" if is_axis(a) else "button",
                "default": default,
            }


def toml_str(s: str) -> str:
    return json.dumps(s, ensure_ascii=False)


def write(channel: Path) -> Path:
    manifest = json.loads((channel / "build_manifest.id").read_text("utf-8-sig"))["Data"]
    version = manifest["Version"]
    lines = [
        f"# Star Citizen {version} ({manifest.get('Branch', '')}): actions of defaultProfile.xml.",
        f"# Generated by dev/catalog.py on {date.today().isoformat()}; do not edit by hand.",
    ]
    seen = set()
    for a in actions(channel):
        if a["id"] in seen:
            continue
        seen.add(a["id"])
        lines += [
            "",
            "[[action]]",
            f"id = {toml_str(a['id'])}",
            f"category = {toml_str(a['category'])}",
            f"name = {toml_str(a['name'])}",
        ]
        if a["kind"] != "button":
            lines.append(f'kind = "{a["kind"]}"')
        if a["default"]:
            lines.append("default = [" + ", ".join(toml_str(d) for d in a["default"]) + "]")
    out = PLUGIN / "catalog" / f"{version}.gob"
    out.write_text("\n".join(lines) + "\n", "utf-8")
    print(f"{out.name}: {len(seen)} actions")
    return out


def version_key(p: Path):
    return [int(x) if x.isdigit() else x for x in p.stem.split(".")]


def renames(new: Path):
    older = sorted(
        (p for p in new.parent.glob("*.gob") if version_key(p) < version_key(new)),
        key=version_key,
    )
    if not older:
        return
    old = {a["id"]: a for a in tomllib.loads(older[-1].read_text("utf-8"))["action"]}
    cur = {a["id"]: a for a in tomllib.loads(new.read_text("utf-8"))["action"]}
    gone = [old[i] for i in old if i not in cur]
    came = [cur[i] for i in cur if i not in old]
    for g in gone:
        for c in came:
            if g["name"] == c["name"] and g["id"].split("/")[0] == c["id"].split("/")[0]:
                print(f"rename candidate since {new.stem}: {g['id']} -> {c['id']}")
    print(f"{len(gone)} gone, {len(came)} new since {older[-1].stem}")


if __name__ == "__main__":
    for arg in sys.argv[1:]:
        renames(write(Path(arg)))
