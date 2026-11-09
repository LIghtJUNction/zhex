#!/usr/bin/env python3
"""Exercise release policy against real, isolated Git histories; no network."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest

import release_tools as release


FIXTURE = {
    "Cargo.toml": '[package]\nname = "zhex"\nversion = "0.1.0"\nedition = "2021"\n',
    "Cargo.lock": '# Root lock format must remain compatible with MSRV.\nversion = 3\n\n'
                  '[[package]]\nname = "zhex"\nversion = "0.1.0"\n',
    "comparison/Cargo.toml": '[package]\nname = "zhex-comparison"\nversion = "0.0.0"\n\n'
                             '[dependencies]\nzhex = { path = ".." }\nhex = "=0.4.3"\n',
    "comparison/Cargo.lock": 'version = 4\n\n[[package]]\nname = "hex"\nversion = "0.4.3"\n'
                             'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
                             'checksum = "preserve this unrelated checksum"\n\n'
                             '[[package]]\nname = "zhex"\nversion = "0.1.0"\n\n'
                             '[[package]]\nname = "zhex-comparison"\nversion = "0.0.0"\n'
                             'dependencies = ["hex", "zhex"]\n',
}


class VersionTests(unittest.TestCase):
    def test_numeric_bumps(self):
        for choice, expected in (("current", "0.9.9"), ("patch", "0.9.10"),
                                 ("minor", "0.10.0"), ("major", "1.0.0")):
            with self.subTest(choice=choice):
                self.assertEqual(release.select_version("0.9.9", choice), expected)
        self.assertEqual(release.select_version("0.9.9", "custom", "0.10.0"), "0.10.0")
        self.assertEqual(release.select_version("0.1.0", "custom", "0.1.0"), "0.1.0")

    def test_invalid_versions_and_downgrades(self):
        for version in ("", "v0.1.0", "0.01.0", "0.1", "0.1.0-alpha", "0.1.0+meta",
                        "0.1.0\n", "0.1.-1", "0.1.0;echo bad", "0.0.9", str(2**64) + ".0.0"):
            with self.subTest(version=version), self.assertRaises(ValueError):
                release.select_version("0.1.0", "custom", version)
        with self.assertRaises(ValueError):
            release.select_version("0.1.0", "patch", "0.2.0")
        with self.assertRaises(ValueError):
            release.select_version("0.1.0", "unknown")
        with self.assertRaises(ValueError):
            release.select_version(f"0.1.{2**64 - 1}", "patch")


class RepositoryTests(unittest.TestCase):
    def setUp(self):
        self.previous_cwd = Path.cwd()
        self.temporary = tempfile.TemporaryDirectory(prefix="zhex-release-test-")
        os.chdir(self.temporary.name)
        self.addCleanup(self.cleanup)
        self.command("init", "--initial-branch=main")
        self.command("config", "user.name", "Release test")
        self.command("config", "user.email", "release-test@example.invalid")
        for name, content in FIXTURE.items():
            Path(name).parent.mkdir(parents=True, exist_ok=True)
            Path(name).write_text(content)
        self.base = self.commit()

    def cleanup(self):
        os.chdir(self.previous_cwd)
        self.temporary.cleanup()

    def command(self, *arguments):
        return subprocess.check_output(["git", *arguments], stderr=subprocess.PIPE, text=True).strip()

    def commit(self):
        self.command("add", ".")
        self.command("commit", "--allow-empty", "-m", "fixture")
        sha = self.command("rev-parse", "HEAD")
        self.command("update-ref", "refs/remotes/origin/main", sha)
        return sha

    def contents(self):
        return {name: Path(name).read_text() for name in release.VERSION_FILES}

    def request(self, version="0.1.0", source=None, extra=False):
        Path(".github").mkdir(exist_ok=True)
        Path(release.REQUEST).write_text(json.dumps({"version": version, "source_sha": source or self.base}))
        if extra:
            Path("unrelated.txt").write_text("An activation commit cannot change this.\n")
        return self.commit()

    def plan(self, sha=None, **kwargs):
        parameters = dict(event="workflow_dispatch", ref="refs/heads/main", branch="main", sha=sha or self.base)
        parameters.update(kwargs)
        return release.plan(**parameters)

    def test_first_release_keeps_current_version(self):
        self.assertEqual(self.plan()["version"], "0.1.0")
        release.apply_version("0.1.0", self.base, "main")
        self.assertEqual(Path("Cargo.toml").read_text(), FIXTURE["Cargo.toml"])
        self.assertEqual(Path("Cargo.lock").read_text(), FIXTURE["Cargo.lock"])
        self.assertEqual(Path("comparison/Cargo.lock").read_text(), FIXTURE["comparison/Cargo.lock"])
        dep = tomllib.loads(Path("comparison/Cargo.toml").read_text())["dependencies"]["zhex"]
        self.assertEqual(dep, {"path": "..", "version": "=0.1.0"})

    def test_bump_updates_only_four_version_fields(self):
        release.apply_version("0.2.0", self.base, "main")
        changed = self.contents()
        self.assertEqual(changed["Cargo.toml"], FIXTURE["Cargo.toml"].replace('version = "0.1.0"', 'version = "0.2.0"'))
        for name in ("Cargo.lock", "comparison/Cargo.lock"):
            self.assertEqual(changed[name], FIXTURE[name].replace('version = "0.1.0"', 'version = "0.2.0"'))
        self.assertEqual(changed["comparison/Cargo.toml"], FIXTURE["comparison/Cargo.toml"].replace(
            'zhex = { path = ".." }', 'zhex = { path = "..", version = "=0.2.0" }'))
        self.assertEqual(tomllib.loads(changed["Cargo.lock"])["version"], 3)

    def test_subsequent_bump_keeps_path_pin_and_other_fields(self):
        text = FIXTURE["comparison/Cargo.toml"].replace(
            '{ path = ".." }', '{ path = "..", version = "=0.1.0", default-features = false }')
        Path("comparison/Cargo.toml").write_text(text)
        sha = self.commit()
        release.apply_version("1.0.0", sha, "main")
        dep = tomllib.loads(Path("comparison/Cargo.toml").read_text())["dependencies"]["zhex"]
        self.assertEqual(dep, {"path": "..", "version": "=1.0.0", "default-features": False})

    def test_inconsistent_lockfiles_or_path_dependency_never_write(self):
        mutations = [
            ("Cargo.lock", lambda text: text.replace('version = "0.1.0"', 'version = "0.0.9"')),
            ("comparison/Cargo.lock", lambda text: text.replace('version = "0.1.0"', 'version = "0.0.9"')),
            ("comparison/Cargo.lock", lambda text: text + '\n[[package]]\nname = "zhex"\nversion = "0.1.0"\n'),
            ("comparison/Cargo.lock", lambda text: text.replace('name = "zhex"', 'source = "registry+wrong"\nname = "zhex"')),
            ("comparison/Cargo.toml", lambda text: text.replace('path = ".."', 'path = "../other"')),
            ("comparison/Cargo.toml", lambda text: text.replace('path = ".."', 'path = "..", version = "=0.0.9"')),
        ]
        for name, mutate in mutations:
            with self.subTest(name=name):
                for path, text in FIXTURE.items():
                    Path(path).write_text(text)
                Path(name).write_text(mutate(FIXTURE[name]))
                sha = self.commit()
                before = self.contents()
                with self.assertRaises(ValueError):
                    release.apply_version("0.2.0", sha, "main")
                self.assertEqual(self.contents(), before)

    def test_existing_tag_cannot_be_reused_or_decreased(self):
        self.command("tag", "v0.10.0")
        for version in ("0.1.0", "0.9.9", "0.10.0"):
            with self.subTest(version=version), self.assertRaises(ValueError):
                release.apply_version(version, self.base, "main")
        self.assertEqual(self.contents(), FIXTURE)
        self.assertEqual(self.plan(bump="custom", explicit="0.10.1")["tag"], "v0.10.1")

    def test_rejects_untrusted_refs_and_events(self):
        for ref in ("refs/heads/topic", "refs/tags/v0.1.0", "refs/pull/1/merge"):
            with self.subTest(ref=ref), self.assertRaises(ValueError):
                self.plan(ref=ref)
        for event in ("pull_request", "pull_request_target", "workflow_run"):
            with self.subTest(event=event), self.assertRaises(ValueError):
                self.plan(event=event)

    def test_rejects_stale_default_head_or_wrong_checkout(self):
        Path("next.txt").write_text("new default head\n")
        advanced = self.commit()
        with self.assertRaises(ValueError):
            self.plan()
        self.command("checkout", "--detach", self.base)
        with self.assertRaises(ValueError):
            self.plan()
        with self.assertRaises(ValueError):
            self.plan(sha=advanced)

    def test_rejects_dirty_worktree_before_writing(self):
        Path("untracked.txt").write_text("preserve me\n")
        with self.assertRaises(ValueError):
            release.apply_version("0.2.0", self.base, "main")
        self.assertEqual(self.contents(), FIXTURE)

    def test_request_binds_single_file_parent_and_event(self):
        sha = self.request()
        result = self.plan(sha, event="push", before=self.base)
        self.assertEqual(result["version"], "0.1.0")
        self.assertEqual(result["base_sha"], sha)
        with self.assertRaises(ValueError):
            self.plan(sha, event="push", before="0" * 40)

    def test_request_rejects_other_changed_files(self):
        sha = self.request(extra=True)
        with self.assertRaisesRegex(ValueError, "only .github/release-request.json"):
            self.plan(sha, event="push", before=self.base)

    def test_request_rejects_wrong_source(self):
        sha = self.request(source="0" * 40)
        with self.assertRaisesRegex(ValueError, "single-parent"):
            self.plan(sha, event="push", before=self.base)

    def test_request_rejects_merge_commit(self):
        self.command("checkout", "-b", "side")
        Path("side.txt").write_text("side\n")
        self.commit()
        self.command("checkout", "main")
        self.request()
        self.command("merge", "--no-ff", "--no-edit", "side")
        sha = self.command("rev-parse", "HEAD")
        self.command("update-ref", "refs/remotes/origin/main", sha)
        with self.assertRaisesRegex(ValueError, "single-parent"):
            self.plan(sha, event="push", before=self.base)

    def test_request_rejects_symlink(self):
        Path(".github").mkdir()
        Path(release.REQUEST).symlink_to("../Cargo.toml")
        sha = self.commit()
        with self.assertRaisesRegex(ValueError, "regular"):
            self.plan(sha, event="push", before=self.base)

    def test_request_rejects_extra_duplicate_or_missing_keys(self):
        Path(".github").mkdir()
        requests = (
            '{"version":"0.1.0","source_sha":"' + self.base + '","unexpected":true}',
            '{"version":"0.1.0","version":"0.2.0","source_sha":"' + self.base + '"}',
            '{"version":"0.1.0"}',
            '[]',
        )
        for contents in requests:
            with self.subTest(contents=contents):
                Path(release.REQUEST).write_text(contents)
                sha = self.commit()
                with self.assertRaises(ValueError):
                    self.plan(sha, event="push", before=self.base)

    def test_resolve_binds_tag_version_commit_and_trusted_history(self):
        self.command("tag", "v0.1.0")
        result = release.resolve("v0.1.0", "refs/remotes/origin/main", self.base)
        self.assertEqual(result, {"sha": self.base, "tag": "v0.1.0", "version": "0.1.0"})
        with self.assertRaises(ValueError):
            release.resolve("v0.1.0", "refs/remotes/origin/main", "0" * 40)
        with self.assertRaises(ValueError):
            release.resolve("v0.1.0", "HEAD", self.base)
        self.command("tag", "v0.2.0")
        with self.assertRaisesRegex(ValueError, "Cargo.toml"):
            release.resolve("v0.2.0", "refs/remotes/origin/main")

    def test_resolve_rejects_tag_outside_default_history(self):
        self.command("checkout", "-b", "untrusted")
        Path("other.txt").write_text("unmerged branch\n")
        self.command("add", ".")
        self.command("commit", "-m", "unmerged")
        self.command("tag", "v0.1.0")
        with self.assertRaisesRegex(ValueError, "outside"):
            release.resolve("v0.1.0", "refs/remotes/origin/main")

    def test_annotated_tag_resolves_to_commit_and_checks_expected_commit(self):
        self.command("tag", "-a", "v0.1.0", "-m", "Annotated release")
        tag_object = self.command("rev-parse", "refs/tags/v0.1.0")
        self.assertNotEqual(tag_object, self.base)
        for expected in ("", self.base):
            with self.subTest(expected=expected):
                result = release.resolve("v0.1.0", "refs/remotes/origin/main", expected)
                self.assertEqual(result["sha"], self.base)
        with self.assertRaises(ValueError):
            release.resolve("v0.1.0", "refs/remotes/origin/main", tag_object)


if __name__ == "__main__":
    unittest.main(verbosity=2)
