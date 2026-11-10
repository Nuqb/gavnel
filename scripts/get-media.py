"""Fetch and verify the pinned LGPL editing runtime (development/packaging only)."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent

def copy_notices(destination):
    shutil.copy2(ROOT / "packaging/media-SOURCES.md", destination / "SOURCES.md")
    for path in (ROOT / "packaging/media-licenses").iterdir():
        shutil.copy2(path, destination / path.name)

def main():
    manifest = json.loads((ROOT / "packaging/media.json").read_text())
    archive = ROOT / "work/tools/media.zip"
    destination = ROOT / "work/media"
    archive.parent.mkdir(parents=True, exist_ok=True)
    if not archive.exists() or hashlib.sha256(archive.read_bytes()).hexdigest() != manifest["sha256"]:
        temporary = archive.with_suffix(".download")
        with urllib.request.urlopen(manifest["url"], timeout=120) as response, temporary.open("wb") as output:
            shutil.copyfileobj(response, output)
        if hashlib.sha256(temporary.read_bytes()).hexdigest() != manifest["sha256"]:
            temporary.unlink()
            raise RuntimeError("Editing runtime checksum mismatch")
        temporary.replace(archive)
    verified = destination / "verified-files.json"
    expected = json.loads((ROOT / "packaging/media-files.json").read_text())
    if expected and all((destination / name).is_file() and hashlib.sha256((destination / name).read_bytes()).hexdigest() == digest for name, digest in expected.items()):
        shutil.copy2(ROOT / "packaging/media.json", destination / "manifest.json")
        copy_notices(destination)
        print(destination)
        return
    with zipfile.ZipFile(archive) as package:
        for entry in package.infolist():
            path = Path(entry.filename)
            if entry.is_dir() or len(path.parts) < 2:
                continue
            relative = Path(*path.parts[1:])
            if relative.is_absolute() or ".." in relative.parts:
                raise RuntimeError("Unsafe editing runtime archive")
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            with package.open(entry) as source, target.open("wb") as output:
                shutil.copyfileobj(source, output)
    shutil.copy2(ROOT / "packaging/media.json", destination / "manifest.json")
    files = {str(path.relative_to(destination)): hashlib.sha256(path.read_bytes()).hexdigest() for path in (destination / "bin").glob("*") if path.is_file()}
    expected = json.loads((ROOT / "packaging/media-files.json").read_text())
    if any(hashlib.sha256((destination / name).read_bytes()).hexdigest() != digest for name, digest in expected.items()):
        raise RuntimeError("Editing runtime file verification failed")
    copy_notices(destination)
    verified.write_text(json.dumps(files, indent=2))
    print(destination)

def copy_runtime(target):
    source = ROOT / "work/media"
    target = target.resolve()
    if target == source.resolve():
        raise ValueError("Choose a separate runtime destination")
    expected = json.loads((ROOT / "packaging/media-files.json").read_text())
    complete = all((target / name).is_file() and hashlib.sha256((target / name).read_bytes()).hexdigest() == digest for name, digest in expected.items())
    if not complete:
        shutil.copytree(source, target, dirs_exist_ok=True)
    copy_notices(target)
    shutil.copy2(ROOT / "packaging/media.json", target / "manifest.json")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--copy-to", type=Path)
    arguments = parser.parse_args()
    main()
    if arguments.copy_to:
        copy_runtime(arguments.copy_to)
