#!/usr/bin/env python3
"""Make the Windows test build, orbital-laser-windows-test.zip, with this checkout's mod in it.

The zip is laid out as the release's own (README-WINDOWS.txt says how to use it: prepare.bat once, then play.bat):

    orbital-laser-windows-test/
        horizon/                         the Orbital Laser build of Horizon for Windows (horizon-client.exe and friends)
        game/horizon-mods/orbital/       the mod (dist/orbital, from tools/build_mod.py)
        game/horizon-mods.txt            the mod switched on
        play.bat, prepare.bat, README-WINDOWS.txt (tools/windows/), LICENSE, NOTICE

The mod (mod.wasm and its content) is the same on every platform, so only the game has to be built for Windows. This
takes the game from a released Windows zip (`--base`: a file, or by default the one on Stink-O/orbital-laser's latest
release, downloaded) and puts the mod from dist/orbital in place of the one in it: nothing is compiled for Windows here.

    python3 tools/build_mod.py                 # first: dist/orbital
    python3 tools/package_windows.py [--base ZIP] [--out ZIP] [--with-recordings]

The Earthquake's two optional recordings (sounds/quake/nhk_alert.ogg and siren.ogg) are someone else's and kept out of
the repository; they are left out of the zip too unless `--with-recordings` (for your own use, not for a release).
Writes the zip and its SHA-256 (beside it, as `<zip>.sha256`).
"""
import argparse, hashlib, shutil, sys, tempfile, tomllib, urllib.request, zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MOD = ROOT / "dist" / "orbital"
WIN = ROOT / "tools" / "windows"
TOP = "orbital-laser-windows-test/"
BASE_URL = "https://github.com/Stink-O/orbital-laser/releases/latest/download/orbital-laser-windows-test.zip"
RECORDINGS = {"assets/orbital/sounds/quake/nhk_alert.ogg", "assets/orbital/sounds/quake/siren.ogg"}
# what this checkout supplies in place of the base zip's own
OURS = ("game/horizon-mods/orbital/", "game/horizon-mods.txt", "play.bat", "prepare.bat", "README-WINDOWS.txt", "LICENSE", "NOTICE")

ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
ap.add_argument("--base", help=f"a released Windows zip to take the game from (default: download {BASE_URL})")
ap.add_argument("--out", default=str(ROOT / "dist" / "orbital-laser-windows-test.zip"))
ap.add_argument("--with-recordings", action="store_true", help="also pack the Earthquake's two recordings, if present")
args = ap.parse_args()

if not (MOD / "mod.wasm").is_file():
    sys.exit(f"no mod at {MOD}: run python3 tools/build_mod.py first")
version = tomllib.loads((MOD / "horizon-mod.toml").read_text())["version"]

with tempfile.TemporaryDirectory() as tmp:
    base = Path(args.base) if args.base else Path(tmp) / "base.zip"
    if not args.base:
        print(f"downloading {BASE_URL}")
        with urllib.request.urlopen(BASE_URL) as r, open(base, "wb") as f:
            shutil.copyfileobj(r, f)
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    part = out.with_suffix(".zip.part")
    game = 0
    with zipfile.ZipFile(base) as src, zipfile.ZipFile(part, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as dst:
        names = src.namelist()
        if TOP + "horizon/horizon-client.exe" not in names:
            sys.exit(f"{base} isn't a Windows test build (no {TOP}horizon/horizon-client.exe)")
        # the game, as released
        for info in src.infolist():
            rel = info.filename[len(TOP):]
            if not info.filename.startswith(TOP) or rel.startswith(OURS):
                continue
            dst.writestr(info, src.read(info), compress_type=zipfile.ZIP_DEFLATED)
            game += 1

        def add(rel, data):
            dst.writestr(zipfile.ZipInfo(TOP + rel, date_time=(2026, 1, 1, 0, 0, 0)), data)

        # Windows' own text files with CRLF line ends (Notepad and cmd both read them)
        def text(path):
            return path.read_bytes().replace(b"\r\n", b"\n").replace(b"\n", b"\r\n")

        for name in ("play.bat", "prepare.bat", "README-WINDOWS.txt"):
            add(name, text(WIN / name))
        add("LICENSE", text(ROOT / "LICENSE"))
        add("NOTICE", text(ROOT / "NOTICE"))
        add("game/horizon-mods.txt", b"mod.orbital:true\n")
        left_out = []
        mod = 0
        for f in sorted(p for p in MOD.rglob("*") if p.is_file()):
            rel = f.relative_to(MOD).as_posix()
            if rel in RECORDINGS and not args.with_recordings:
                left_out.append(rel)
                continue
            add("game/horizon-mods/orbital/" + rel, f.read_bytes())
            mod += 1
    part.replace(out)

digest = hashlib.sha256(out.read_bytes()).hexdigest()
Path(str(out) + ".sha256").write_text(f"{digest}  {out.name}\n")
print(f"wrote {out} ({out.stat().st_size / 1e6:.1f} MB): the game's {game} files from {base.name if args.base else 'the release'}, "
      f"the mod {version} ({mod} files)")
if left_out:
    print("left out (not ours to give away): " + ", ".join(left_out))
print(f"sha256 {digest}")
