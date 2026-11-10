"""Build a license inventory from the locked Windows dependency graph."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess


def selected_packages(metadata):
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    selected, pending = set(), [metadata["resolve"]["root"]]
    while pending:
        identity = pending.pop()
        if identity in selected:
            continue
        selected.add(identity)
        pending.extend(dep["pkg"] for dep in nodes[identity]["deps"]
                       if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]))
    return sorted((p for p in metadata["packages"] if p["id"] in selected), key=lambda p: (p["name"], p["version"]))


def license_files(package):
    root = Path(package["manifest_path"]).parent
    result = set()
    if package.get("license_file"):
        result.add(root / package["license_file"])
    for candidate in root.rglob("*"):
        if candidate.is_file() and (candidate.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "OFL")) or any(part.upper() in ("LICENSES", "LICENSE", "LICENCES") for part in candidate.relative_to(root).parts[:-1])):
            result.add(candidate)
    if not result:
        for ancestor in list(root.parents)[:3]:
            result.update(p for p in ancestor.glob("*") if p.is_file() and p.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE")))
            if result:
                break
    if not result:
        bundled = Path("packaging/licenses") / f'{package["name"]}-{package["version"]}'
        if bundled.is_dir():
            manifest = json.loads(Path("packaging/licenses/provenance.json").read_text(encoding="utf-8"))
            for entry in manifest[bundled.name]:
                source = bundled / entry["file"]
                if hashlib.sha256(source.read_bytes()).hexdigest() != entry["sha256"]:
                    raise ValueError(f"Pinned license checksum mismatch: {source}")
                result.add(source)
    return sorted(result)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("payload", type=Path)
    args = parser.parse_args()
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc"], text=True, encoding="utf-8"))
    inventory, missing = [], []
    for package in selected_packages(metadata):
        if package["id"] == metadata["resolve"]["root"]:
            continue
        record = {key: package.get(key) for key in ("name", "version", "license", "repository")}
        inventory.append(record)
        files = license_files(package)
        if not files:
            missing.append(f'{package["name"]} {package["version"]}')
            continue
        destination = args.payload / "licenses" / f'{package["name"]}-{package["version"]}'
        destination.mkdir(parents=True, exist_ok=True)
        for index, source in enumerate(files):
            shutil.copy2(source, destination / f"{index:02}-{source.name}")
    if missing:
        raise SystemExit("Missing dependency license files: " + ", ".join(missing))
    fonts = args.payload / "licenses" / "bundled-fonts"
    fonts.mkdir(parents=True, exist_ok=True)
    for source in Path("assets/fonts").glob("*.txt"):
        shutil.copy2(source, fonts / source.name)
    (args.payload / "DEPENDENCIES.json").write_text(json.dumps(inventory, indent=2) + "\n", encoding="utf-8")
    (args.payload / "DEPENDENCIES.txt").write_text("Gavnel Windows release dependency inventory\nExact versions from Cargo.lock, normal and build dependencies.\n\n" + "\n".join(f'{p["name"]} {p["version"]} | {p["license"]} | {p["repository"] or ""}' for p in inventory) + "\n", encoding="utf-8")
    print(f"Packaged license notices for {len(inventory)} dependencies and bundled fonts.")


if __name__ == "__main__":
    main()
