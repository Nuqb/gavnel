"""Release safety checks run without GitHub credentials, git mutations, or installers."""
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("release_tools", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.tag = "v1.2.3"
        self.notes = self.root / "docs/releases/v1.2.3.md"
        self.notes.parent.mkdir(parents=True)
        self.notes.write_text("# Gavnel v1.2.3\n\n- Fixed capture.\n", encoding="utf-8")
        (self.root / "Cargo.toml").write_text('[package]\nname = "gavnel"\nversion = "1.2.3"\n')
        (self.root / "Cargo.lock").write_text('version = 4\n[[package]]\nname = "gavnel"\nversion = "1.2.3"\n')
        (self.root / "CHANGELOG.md").write_text("# Changelog\n\n## 1.2.3\n\n- Fixed capture.\n")
        self.installer = self.root / "Gavnel-Setup-1.2.3-x64.exe"
        self.installer.write_bytes(b"MZverified installer fixture")
        self.sums = self.root / "SHA256SUMS.txt"
        self.sums.write_text(f"{hashlib.sha256(self.installer.read_bytes()).hexdigest()}  {self.installer.name}\n")
        self.files = (self.installer, self.sums)
        self.root_patch = patch.object(release, "ROOT", self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)
        self.run_patch = patch.object(release, "run", return_value="fixture-sha")
        self.run_patch.start()
        self.addCleanup(self.run_patch.stop)

    def draft(self, assets=None, draft=True):
        return {"draft": draft, "prerelease": False, "body": self.notes.read_text(),
                "html_url": "https://example.test/release", "assets": assets or []}

    def remote_assets(self):
        return [{"name": path.name, "size": path.stat().st_size,
                 "digest": "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()}
                for path in self.files]

    def download(self, *arguments):
        self.assertEqual(arguments[:2], ("release", "download"))
        directory = Path(arguments[arguments.index("--dir") + 1])
        for path in self.files:
            (directory / path.name).write_bytes(path.read_bytes())

    def test_stable_versions_compare_numerically_and_reject_noncanonical_tags(self):
        self.assertGreater(release.version_tuple("v1.10.0"), release.version_tuple("v1.9.9"))
        for tag in ("1.2.3", "v01.2.3", "v1.2.3-beta", "v1.2", "v1.2.3/other"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.version_tuple(tag)

    def test_preparation_requires_matching_manifest_lockfile_notes_and_changelog(self):
        self.assertEqual(release.source_notes(self.tag), self.notes)
        (self.root / "Cargo.lock").write_text('[[package]]\nname="gavnel"\nversion="1.2.2"\n')
        with self.assertRaisesRegex(ValueError, "must match"):
            release.source_notes(self.tag)

    def test_missing_release_notes_and_changelog_fail_before_building(self):
        self.notes.unlink()
        with self.assertRaises(FileNotFoundError):
            release.source_notes(self.tag)
        self.notes.write_text("# Release\n\n- Fix\n")
        (self.root / "CHANGELOG.md").write_text("## Unreleased\n")
        with self.assertRaisesRegex(ValueError, "CHANGELOG"):
            release.source_notes(self.tag)

    def test_published_release_is_never_available_for_rebuilding(self):
        with patch.object(release, "api", return_value=self.draft(draft=False)), self.assertRaisesRegex(ValueError, "already published"):
            release.check_release_available(self.tag)

    def test_older_version_cannot_become_latest(self):
        with patch.object(release, "release_info", return_value=None), patch.object(release, "api", return_value={"tag_name": "v1.3.0"}), self.assertRaisesRegex(ValueError, "newer"):
            release.check_release_available(self.tag)

    def test_draft_lookup_falls_back_to_paginated_authenticated_listing(self):
        draft = {"tag_name": self.tag, "draft": True, "assets": []}
        pages = '[[{"tag_name":"v1.2.4"}], [{"tag_name":"v1.2.3","draft":true,"assets":[]}]]'
        with patch.object(release, "api", return_value=None), patch.object(release, "gh", return_value=pages):
            self.assertEqual(release.release_info(self.tag), draft)

    def test_published_lookup_does_not_need_release_listing(self):
        published = self.draft(draft=False)
        with patch.object(release, "api", return_value=published), patch.object(release, "gh") as gh:
            self.assertEqual(release.release_info(self.tag), published)
        gh.assert_not_called()

    def test_absent_tag_is_not_confused_with_another_draft(self):
        with patch.object(release, "api", return_value=None), patch.object(release, "gh", return_value='[[{"tag_name":"v1.2.4","draft":true}]]'):
            self.assertIsNone(release.release_info(self.tag))

    def test_mismatched_tag_commit_fails_preflight(self):
        with patch.object(release, "run", side_effect=["tagged", "other"]), self.assertRaisesRegex(ValueError, "does not match"):
            release.preflight(self.tag)

    def test_installer_tampering_is_rejected(self):
        self.assertEqual(release.verified_files(self.tag, self.root), self.files)
        self.installer.write_bytes(b"MZtampered")
        with self.assertRaisesRegex(ValueError, "does not match"):
            release.verified_files(self.tag, self.root)

    def test_non_executable_payload_is_rejected(self):
        self.installer.write_bytes(b"<html>download error</html>")
        with self.assertRaisesRegex(ValueError, "not a Windows"):
            release.verified_files(self.tag, self.root)

    def test_resume_uploads_only_missing_draft_assets(self):
        draft = self.draft([{"name": self.installer.name}])
        with patch.object(release, "api", return_value=draft), patch.object(release, "gh") as gh:
            release.stage_draft(self.tag, self.files, self.notes)
        gh.assert_called_once_with("release", "upload", self.tag, str(self.sums), "--repo", release.REPOSITORY)

    def test_published_missing_assets_are_never_added(self):
        with patch.object(release, "api", return_value=self.draft(draft=False)), patch.object(release, "gh") as gh:
            with self.assertRaisesRegex(ValueError, "never changed"):
                release.stage_draft(self.tag, self.files, self.notes)
        gh.assert_not_called()

    def test_differing_notes_require_review(self):
        draft = self.draft()
        draft["body"] = "Other notes"
        with patch.object(release, "api", return_value=draft), self.assertRaisesRegex(ValueError, "notes differ"):
            release.stage_draft(self.tag, self.files, self.notes)

    def test_unexpected_asset_blocks_publication(self):
        draft = self.draft([{"name": "unexpected.exe"}])
        with patch.object(release, "api", return_value=draft), self.assertRaisesRegex(ValueError, "unexpected assets"):
            release.stage_draft(self.tag, self.files, self.notes)

    def test_remote_download_and_github_digests_must_match(self):
        draft = self.draft(self.remote_assets())
        with patch.object(release, "api", return_value=draft), patch.object(release, "gh", side_effect=self.download):
            self.assertEqual(release.verify_remote(self.tag, self.files), draft)
            draft["assets"][0]["digest"] = None
            with self.assertRaisesRegex(ValueError, "digest or size"):
                release.verify_remote(self.tag, self.files)

    def test_remote_bytes_cannot_differ_even_with_valid_downloaded_checksums(self):
        draft = self.draft(self.remote_assets())
        def changed_download(*arguments):
            self.download(*arguments)
            directory = Path(arguments[arguments.index("--dir") + 1])
            payload = b"MZanother valid installer"
            (directory / self.installer.name).write_bytes(payload)
            (directory / self.sums.name).write_text(f"{hashlib.sha256(payload).hexdigest()}  {self.installer.name}")
        with patch.object(release, "api", return_value=draft), patch.object(release, "gh", side_effect=changed_download):
            with self.assertRaisesRegex(ValueError, "differs from the tested"):
                release.verify_remote(self.tag, self.files)

    def test_failed_remote_verification_prevents_publication(self):
        with patch.object(release, "api", return_value={"sha": "fixture-sha"}), patch.object(release, "stage_draft"), patch.object(release, "verify_remote", side_effect=ValueError("bad digest")), patch.object(release, "gh") as gh:
            with self.assertRaisesRegex(ValueError, "bad digest"):
                release.publish(self.tag, self.root)
        gh.assert_not_called()

    def test_retry_after_publication_never_edits_the_release(self):
        published = self.draft(self.remote_assets(), draft=False)
        with patch.object(release, "stage_draft"), patch.object(release, "verify_remote", return_value=published), patch.object(release, "api", side_effect=[{"sha": "fixture-sha"}, published]), patch.object(release, "gh") as gh:
            release.publish(self.tag, self.root)
        gh.assert_not_called()

    def test_moved_tag_cannot_publish_a_previously_tested_artifact(self):
        with patch.object(release, "api", return_value={"sha": "other-sha"}), patch.object(release, "stage_draft") as stage:
            with self.assertRaisesRegex(ValueError, "tested commit"):
                release.publish(self.tag, self.root)
        stage.assert_not_called()

    def test_verified_draft_is_published_only_after_checks_and_newer_version_check(self):
        draft = self.draft(self.remote_assets())
        published = self.draft(self.remote_assets(), draft=False)
        metadata = [{"sha": "fixture-sha"}, {"tag_name": "v1.2.2"}, published]
        with patch.object(release, "stage_draft"), patch.object(release, "verify_remote", return_value=draft), patch.object(release, "api", side_effect=metadata), patch.object(release, "gh") as gh:
            release.publish(self.tag, self.root)
        gh.assert_called_once_with("release", "edit", self.tag, "--repo", release.REPOSITORY, "--draft=false", "--latest")

    def test_newer_concurrent_release_prevents_older_draft_becoming_latest(self):
        metadata = [{"sha": "fixture-sha"}, {"tag_name": "v1.2.4"}]
        with patch.object(release, "stage_draft"), patch.object(release, "verify_remote", return_value=self.draft()), patch.object(release, "api", side_effect=metadata), patch.object(release, "gh") as gh:
            with self.assertRaisesRegex(ValueError, "newer stable release"):
                release.publish(self.tag, self.root)
        gh.assert_not_called()

    def test_dirty_checkout_stops_before_any_github_or_push_operation(self):
        with patch.object(release, "run", return_value=" M Cargo.toml"), patch.object(release, "gh") as gh:
            with self.assertRaisesRegex(ValueError, "working tree must be clean"):
                release.ship(self.tag, False)
        gh.assert_not_called()

    def test_resume_refuses_a_remote_tag_for_another_commit(self):
        responses = ["", "main", "fixture-sha", "remote-tag", "", "different-sha"]
        with patch.object(release, "run", side_effect=responses) as command, patch.object(release, "gh", return_value=release.REPOSITORY), patch.object(release, "check_release_available"), patch.object(release, "wait_run") as wait:
            with self.assertRaisesRegex(ValueError, "another commit"):
                release.ship(self.tag, True)
        self.assertFalse(any(call.args[1] in ("push", "tag") for call in command.call_args_list))
        wait.assert_not_called()

    def test_shipping_pins_push_and_tag_to_the_commit_that_passed_ci(self):
        responses = ["", "main", "fixture-sha", "", "", "", "", ""]
        with patch.object(release, "run", side_effect=responses) as command, patch.object(release, "gh", return_value=release.REPOSITORY), patch.object(release, "check_release_available"), patch.object(release, "wait_run"), patch.object(release, "api", return_value=self.draft(draft=False)):
            release.ship(self.tag, False)
        command.assert_any_call("git", "push", "origin", "fixture-sha:refs/heads/main")
        command.assert_any_call("git", "tag", "-a", self.tag, "fixture-sha", "-m", f"Gavnel {self.tag}")

    def test_failed_ci_requires_explicit_resume(self):
        failed = {"databaseId": 42, "status": "completed", "conclusion": "failure"}
        with patch.object(release, "find_run", return_value=failed), patch.object(release, "gh") as gh:
            with self.assertRaisesRegex(RuntimeError, "CI failed"):
                release.wait_run("CI", "fixture-sha", "main")
        gh.assert_not_called()

    def test_rerun_waits_for_a_new_attempt_instead_of_reading_stale_failure(self):
        responses = ['{"attempt": 1}', "", '{"attempt": 1}', '{"attempt": 2}']
        with patch.object(release, "gh", side_effect=responses), patch.object(release.time, "sleep") as sleep:
            release.rerun_failed("42")
        sleep.assert_called_once_with(5)

    def test_authentication_and_network_errors_are_not_treated_as_missing_releases(self):
        with patch.object(release, "gh", side_effect=RuntimeError("HTTP 403 denied")), self.assertRaises(RuntimeError):
            release.api("releases/latest")
        with patch.object(release, "gh", side_effect=RuntimeError("HTTP 404 not found")):
            self.assertIsNone(release.api("releases/latest"))


if __name__ == "__main__":
    unittest.main()
