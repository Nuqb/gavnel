"""Prepare the release handoff and verify immutable assets before publication.

Local use goes through release.ps1. CI uses preflight and publish directly.
Commands use argument arrays, never a shell; importing this module has no effects.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time
import tomllib

ROOT = Path(__file__).resolve().parent.parent
REPOSITORY = "Nuqb/gavnel"
STABLE = re.compile(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")


def run(*arguments):
    result = subprocess.run(arguments, cwd=ROOT, capture_output=True, text=True,
                            encoding="utf-8", errors="replace")
    if result.returncode:
        raise RuntimeError(f"{' '.join(arguments)} failed:\n{result.stderr.strip()}\n{result.stdout.strip()}")
    return result.stdout.strip()


def gh(*arguments):
    return run("gh", *arguments)


def api(resource):
    try:
        return json.loads(gh("api", f"repos/{REPOSITORY}/{resource}"))
    except RuntimeError as error:
        if "HTTP 404" in str(error):
            return None
        raise


def version_tuple(tag):
    match = STABLE.fullmatch(tag)
    if not match:
        raise ValueError("Use a stable vX.Y.Z tag without leading zeroes.")
    return tuple(map(int, match.groups()))


def source_notes(tag):
    version_tuple(tag)
    version = tag[1:]
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    package = manifest["package"]
    locked = [p for p in lock["package"] if p["name"] == package["name"] and "source" not in p]
    if package["version"] != version or len(locked) != 1 or locked[0]["version"] != version:
        raise ValueError("The tag, Cargo.toml, and root Cargo.lock version must match.")
    notes = ROOT / "docs" / "releases" / f"{tag}.md"
    body = notes.read_text(encoding="utf-8").strip()
    if not any(line.strip() and not line.startswith("#") for line in body.splitlines()):
        raise ValueError("Release notes must contain user-facing changes.")
    changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    if not re.search(rf"(?m)^## {re.escape(version)}\s*$", changelog):
        raise ValueError(f"Add a CHANGELOG.md section for {version}.")
    return notes


def check_release_available(tag, allow_published=False):
    existing = api(f"releases/tags/{tag}")
    if existing and not existing["draft"]:
        if allow_published:
            return
        raise ValueError(f"{tag} is already published. Choose a new version; assets are never replaced.")
    latest = api("releases/latest")
    if latest and version_tuple(tag) <= version_tuple(latest["tag_name"]):
        raise ValueError("A new stable release must be newer than the current latest release.")


def preflight(tag):
    source_notes(tag)
    tagged = run("git", "rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}")
    if tagged != run("git", "rev-parse", "HEAD"):
        raise ValueError("The checked-out commit does not match the release tag.")
    run("git", "merge-base", "--is-ancestor", tagged, "origin/main")
    check_release_available(tag)
    print(f"Validated {tag}: version, lockfile, notes, changelog, main ancestry, and release availability.")


def verified_files(tag, directory):
    version_tuple(tag)
    installer = directory / f"Gavnel-Setup-{tag[1:]}-x64.exe"
    sums = directory / "SHA256SUMS.txt"
    if not installer.is_file() or installer.stat().st_size < 2:
        raise ValueError("The tested Windows installer is missing or empty.")
    with installer.open("rb") as stream:
        if stream.read(2) != b"MZ":
            raise ValueError("The release asset is not a Windows executable.")
        stream.seek(0)
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    expected = f"{digest}  {installer.name}"
    if sums.read_text(encoding="ascii").strip() != expected:
        raise ValueError("SHA256SUMS.txt does not match the tested installer.")
    return installer, sums


def stage_draft(tag, files, notes):
    existing = api(f"releases/tags/{tag}")
    if not existing:
        gh("release", "create", tag, "--repo", REPOSITORY, "--verify-tag", "--draft",
           "--target", run("git", "rev-parse", "HEAD"), "--title", f"Gavnel {tag}",
           "--notes-file", str(notes))
        existing = api(f"releases/tags/{tag}")
    if not existing or existing.get("prerelease"):
        raise ValueError("Expected a stable release draft.")
    if (existing.get("body") or "").strip() != notes.read_text(encoding="utf-8").strip():
        raise ValueError("The existing release notes differ from the tagged notes. Review the draft.")
    expected = {path.name for path in files}
    present = {asset["name"] for asset in existing["assets"]}
    if present - expected:
        raise ValueError("The release contains unexpected assets. Review the draft before resuming.")
    for path in files:
        if path.name not in present:
            if not existing["draft"]:
                raise ValueError("Published release assets are never changed.")
            gh("release", "upload", tag, str(path), "--repo", REPOSITORY)


def verify_remote(tag, files):
    release = api(f"releases/tags/{tag}")
    expected = {path.name for path in files}
    if not release or {asset["name"] for asset in release["assets"]} != expected:
        raise ValueError("The release asset list does not match the tested artifact.")
    with tempfile.TemporaryDirectory(prefix="gavnel-release-") as temporary:
        gh("release", "download", tag, "--repo", REPOSITORY, "--dir", temporary)
        downloaded = Path(temporary)
        verified_files(tag, downloaded)
        for path in files:
            if (downloaded / path.name).read_bytes() != path.read_bytes():
                raise ValueError(f"Uploaded {path.name} differs from the tested artifact; it will not be replaced.")
    for asset in release["assets"]:
        path = next(path for path in files if path.name == asset["name"])
        digest = "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
        if asset.get("digest") != digest or asset.get("size") != path.stat().st_size:
            raise ValueError(f"GitHub digest or size is missing or incorrect for {path.name}. Retry publication.")
    return release


def publish(tag, directory):
    notes = source_notes(tag)
    commit = api(f"commits/{tag}")
    if not commit or commit["sha"] != run("git", "rev-parse", "HEAD"):
        raise ValueError("The release tag no longer points to the tested commit.")
    files = verified_files(tag, directory)
    stage_draft(tag, files, notes)
    release = verify_remote(tag, files)
    if release["draft"]:
        latest = api("releases/latest")
        if latest and version_tuple(tag) <= version_tuple(latest["tag_name"]):
            raise ValueError("A newer stable release was published while this release was building.")
        gh("release", "edit", tag, "--repo", REPOSITORY, "--draft=false", "--latest")
    published = api(f"releases/tags/{tag}")
    if not published or published["draft"] or published["prerelease"]:
        raise ValueError("The release is not published as stable.")
    print(f"Published and verified: {published['html_url']}")


def find_run(workflow, sha, branch):
    deadline = time.monotonic() + 180
    while time.monotonic() < deadline:
        runs = json.loads(gh("run", "list", "--repo", REPOSITORY, "--workflow", workflow,
                             "--commit", sha, "--branch", branch, "--event", "push", "--limit", "30",
                             "--json", "databaseId,status,conclusion,headBranch"))
        if runs:
            return runs[0]
        time.sleep(5)
    raise RuntimeError(f"No {workflow} run appeared for {sha}. Check Actions and resume with -Resume.")


def rerun_failed(identity):
    previous = json.loads(gh("run", "view", identity, "--repo", REPOSITORY, "--json", "attempt"))["attempt"]
    gh("run", "rerun", identity, "--repo", REPOSITORY, "--failed")
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        attempt = json.loads(gh("run", "view", identity, "--repo", REPOSITORY, "--json", "attempt"))["attempt"]
        if attempt > previous:
            return
        time.sleep(5)
    raise RuntimeError("GitHub has not started the new attempt yet. Check Actions before resuming.")


def wait_run(workflow, sha, branch, resume=False):
    workflow_run = find_run(workflow, sha, branch)
    identity = str(workflow_run["databaseId"])
    if workflow_run["status"] == "completed" and workflow_run["conclusion"] != "success":
        if not resume:
            raise RuntimeError(f"{workflow} failed. Review run {identity}, fix the issue, or use -Resume for a transient failure.")
        rerun_failed(identity)
    print(f"Waiting for {workflow}: https://github.com/{REPOSITORY}/actions/runs/{identity}", flush=True)
    gh("run", "watch", identity, "--repo", REPOSITORY, "--exit-status", "--interval", "10")


def ship(tag, resume):
    source_notes(tag)
    if run("git", "status", "--porcelain"):
        raise ValueError("Commit the reviewed changes before releasing. The working tree must be clean.")
    if run("git", "branch", "--show-current") != "main":
        raise ValueError("Run releases from main.")
    gh("auth", "status")
    if gh("repo", "view", "--json", "nameWithOwner", "--jq", ".nameWithOwner").lower() != REPOSITORY.lower():
        raise ValueError(f"The checkout must point to {REPOSITORY}.")
    check_release_available(tag, allow_published=resume)
    sha = run("git", "rev-parse", "HEAD")
    remote_tag = run("git", "ls-remote", "origin", f"refs/tags/{tag}", f"refs/tags/{tag}^{{}}")
    if remote_tag:
        if not resume:
            raise ValueError("The remote tag exists. Use -Resume to continue its existing release.")
        run("git", "fetch", "origin", f"refs/tags/{tag}:refs/tags/{tag}")
        if run("git", "rev-parse", f"refs/tags/{tag}^{{commit}}") != sha:
            raise ValueError("The remote release tag points to another commit. Check out that commit before resuming.")
    else:
        run("git", "push", "origin", f"{sha}:refs/heads/main")
        wait_run("ci.yml", sha, "main", resume)
        local = run("git", "tag", "--list", tag)
        if local and run("git", "rev-parse", f"refs/tags/{tag}^{{commit}}") != sha:
            raise ValueError("The local tag points to another commit; tags are never moved.")
        if not local:
            run("git", "tag", "-a", tag, sha, "-m", f"Gavnel {tag}")
        run("git", "push", "origin", f"refs/tags/{tag}")
    wait_run("release.yml", sha, tag, resume)
    release = api(f"releases/tags/{tag}")
    if not release or release["draft"] or release["prerelease"]:
        raise RuntimeError("The workflow finished without publishing a stable release. Inspect Actions before retrying.")
    print(f"Release complete: {release['html_url']}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for command in ("preflight", "publish", "ship"):
        child = commands.add_parser(command)
        child.add_argument("--tag", required=True)
        if command == "publish":
            child.add_argument("--directory", type=Path, required=True)
        if command == "ship":
            child.add_argument("--resume", action="store_true")
    args = parser.parse_args()
    try:
        if args.command == "preflight":
            preflight(args.tag)
        elif args.command == "publish":
            publish(args.tag, args.directory)
        else:
            ship(args.tag, args.resume)
    except (OSError, ValueError, RuntimeError) as error:
        print(f"Release stopped: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
