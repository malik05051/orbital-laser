#!/usr/bin/env python3
"""Make the mod's release package, dist/orbital-laser-mod.zip and dist/SHA256SUMS, from this checkout's dist/orbital.

The fork's Horizon launcher (malik05051/horizon) installs and updates the mod from the latest release of
malik05051/orbital-laser: tag `vX.Y.Z`, with these two files. Keep the layout as it is:

    orbital-laser-mod.zip
        orbital/                 exactly what goes in the game folder's horizon-mods/orbital/
            horizon-mod.toml     its version is the tag's, without the v
            mod.wasm, assets/, data/
    SHA256SUMS                   "<sha256>  orbital-laser-mod.zip"

    python3 tools/build_mod.py         # first: dist/orbital
    python3 tools/package_mod.py [--out DIR]

The player's own recordings (sounds/quake/nhk_alert.ogg, siren.ogg, eew.ogg, major.ogg, alert.ogg, cancel.ogg and
their far versions) are someone else's: they are always left out, and the events build_mod.py pointed at them point
back at their synthesised sounds (nhk_alert and siren have none: they are silent without the recordings). The zip's
entries are sorted and dated alike, so the same mod gives the same zip.
"""
import argparse, hashlib, sys, tomllib, zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MOD = ROOT / "dist" / "orbital"
NAME = "orbital-laser-mod.zip"
TOP = "orbital/"
RECORDED = ("nhk_alert", "siren", "eew", "eew_far", "major", "major_far", "alert", "alert_far", "cancel", "cancel_far")
RECORDINGS = {f"assets/orbital/sounds/quake/{n}.ogg" for n in RECORDED}
# (the events with a synthesised stand-in, pointed back at it)
SYNTH = (b"eew", b"eew_far", b"major", b"major_far", b"alert", b"alert_far", b"cancel", b"cancel_far")


def package(mod: Path, out_dir: Path) -> tuple[Path, str, str, list[str]]:
    """The zip and SHA256SUMS written in `out_dir` from the mod folder `mod`: (zip, version, digest, left out)."""
    version = tomllib.loads((mod / "horizon-mod.toml").read_text())["version"]
    out_dir.mkdir(parents=True, exist_ok=True)
    out = out_dir / NAME
    part = out.with_suffix(".zip.part")
    left_out = []
    with zipfile.ZipFile(part, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for f in sorted(p for p in mod.rglob("*") if p.is_file()):
            rel = f.relative_to(mod).as_posix()
            if rel in RECORDINGS:
                left_out.append(rel)
                continue
            data = f.read_bytes()
            if rel == "assets/orbital/sounds.json":
                for n in SYNTH:
                    data = data.replace(b'"orbital:quake/' + n + b'"', b'"orbital:quake/synth/' + n + b'"')
            info = zipfile.ZipInfo(TOP + rel, date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            z.writestr(info, data)
    part.replace(out)
    digest = hashlib.sha256(out.read_bytes()).hexdigest()
    (out_dir / "SHA256SUMS").write_text(f"{digest}  {NAME}\n")
    return out, version, digest, left_out


def check(zip_path: Path) -> None:
    """Fails unless the zip is laid out as the launcher reads it: only orbital/... paths, its manifest in, no
    recording, every event pointed at a sound the zip has (or at a recording, silent without it)."""
    with zipfile.ZipFile(zip_path) as z:
        names = z.namelist()
        bad = [n for n in names if not n.startswith(TOP) or ".." in n.split("/") or n.startswith("/")]
        assert not bad, f"paths outside {TOP}: {bad[:5]}"
        assert TOP + "horizon-mod.toml" in names and TOP + "mod.wasm" in names, "no manifest or mod.wasm"
        rec = [n for n in names if n[len(TOP):] in RECORDINGS]
        assert not rec, f"recordings packed: {rec}"
        sj = z.read(TOP + "assets/orbital/sounds.json")
        for n in SYNTH:
            assert b'"orbital:quake/' + n + b'"' not in sj, f"quake.{n.decode()} still points at a recording"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--out", default=str(ROOT / "dist"), help="the folder the zip and SHA256SUMS go in (default dist/)")
    args = ap.parse_args()
    if not (MOD / "mod.wasm").is_file():
        sys.exit(f"no mod at {MOD}: run python3 tools/build_mod.py first")
    out, version, digest, left_out = package(MOD, Path(args.out))
    check(out)
    print(f"wrote {out} ({out.stat().st_size / 1e6:.1f} MB), Orbital Laser {version}: release it as v{version}")
    print(f"SHA256SUMS: {digest}  {NAME}")
    if left_out:
        print("left out (not ours to give away): " + ", ".join(left_out))


if __name__ == "__main__":
    main()
