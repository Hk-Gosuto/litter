#!/usr/bin/env python3
"""Package an Xcode archive for local re-signing, including nested Watch apps."""

import argparse
import json
import plistlib
import shutil
import subprocess
import tempfile
import zipfile
from pathlib import Path

MACH_O_MAGICS = {
    bytes.fromhex(value)
    for value in ("feedface", "cefaedfe", "feedfacf", "cffaedfe",
                  "cafebabe", "bebafeca", "cafebabf", "bfbafeca")
}


def package(archive: Path, output: Path) -> None:
    source = archive / "Products/Applications/Litter.app"
    if not source.is_dir():
        raise SystemExit(f"Missing archived app: {source}")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="litter-ipa-", dir=output.parent) as temporary:
        payload = Path(temporary) / "Payload"
        app = payload / "Litter.app"
        shutil.copytree(source, app, symlinks=True)
        for path in list(app.rglob("_CodeSignature")):
            if path.is_dir() and not path.is_symlink():
                shutil.rmtree(path)
        for path in app.rglob("*.mobileprovision"):
            path.unlink()
        binaries = 0
        for path in app.rglob("*"):
            if not path.is_file() or path.is_symlink():
                continue
            with path.open("rb") as stream:
                if stream.read(4) not in MACH_O_MAGICS:
                    continue
            subprocess.run(["codesign", "--remove-signature", str(path)],
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            verification = subprocess.run(["codesign", "-dv", str(path)],
                                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            if b"code object is not signed at all" not in verification.stderr:
                raise SystemExit(f"Could not verify unsigned Mach-O: {path}")
            binaries += 1
        packaged = Path(temporary) / "Litter.ipa"
        subprocess.run(["ditto", "-c", "-k", "--norsrc", "--keepParent",
                        str(payload), str(packaged)], check=True)
        with zipfile.ZipFile(packaged) as ipa:
            if ipa.testzip() is not None:
                raise SystemExit("IPA archive failed integrity verification")
            names = ipa.namelist()
            if "Payload/Litter.app/Watch/LitterWatch.app/LitterWatch" not in names:
                raise SystemExit("Watch companion missing from IPA")
            if any("/_CodeSignature/" in name or name.endswith(".mobileprovision") for name in names):
                raise SystemExit("Unexpected signing metadata in IPA")
            info = plistlib.loads(ipa.read("Payload/Litter.app/Info.plist"))
        packaged.replace(output)
        print(json.dumps({"ipa": str(output), "bytes": output.stat().st_size,
                          "version": info["CFBundleShortVersionString"],
                          "build": info["CFBundleVersion"],
                          "unsigned_mach_o_files": binaries, "watch_companion": True}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("output", type=Path)
    arguments = parser.parse_args()
    package(arguments.archive.resolve(), arguments.output.resolve())
