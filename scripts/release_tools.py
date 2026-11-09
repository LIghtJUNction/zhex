#!/usr/bin/env python3
"""Validate release intent and edit only zhex's version metadata (CI only)."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tomllib

REQUEST = ".github/release-request.json"
VERSION_FILES = ("Cargo.toml", "Cargo.lock", "comparison/Cargo.toml", "comparison/Cargo.lock")
SEMVER = r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


def version_parts(version):
    if not isinstance(version, str) or not re.fullmatch(SEMVER, version):
        raise ValueError("Version must be X.Y.Z, without prefix, leading zero, or prerelease")
    parts = tuple(map(int, version.split(".")))
    if any(part > 2**64 - 1 for part in parts):
        raise ValueError("Version component exceeds Cargo's u64 range")
    return parts


def require_sha(sha):
    if not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("Expected a full lowercase 40-character commit SHA")


def current_version():
    package = tomllib.loads(Path("Cargo.toml").read_text())["package"]
    if package["name"] != "zhex":
        raise ValueError("Release manifest must describe zhex")
    version_parts(package["version"])
    return package["version"]


def select_version(current, bump, explicit=""):
    old = version_parts(current)
    if bump == "custom":
        target = explicit
    else:
        if explicit:
            raise ValueError("Explicit version is only accepted with the custom choice")
        if bump == "current":
            target = current
        elif bump in ("patch", "minor", "major"):
            index = {"major": 0, "minor": 1, "patch": 2}[bump]
            new = list(old)
            new[index] += 1
            new[index + 1:] = [0] * (2 - index)
            target = ".".join(map(str, new))
        else:
            raise ValueError("Unknown version bump choice")
    if version_parts(target) < old:
        raise ValueError("Release versions cannot decrease")
    return target


def require_checkout(sha, branch):
    require_sha(sha)
    git("check-ref-format", f"refs/heads/{branch}")
    if git("rev-parse", "HEAD") != sha:
        raise ValueError("Checkout does not match the selected event commit")
    if git("rev-parse", f"refs/remotes/origin/{branch}") != sha:
        raise ValueError("Default branch advanced; select its current head and retry")
    if git("status", "--porcelain"):
        raise ValueError("Release preparation requires a clean checkout")


def require_new_tag(version):
    target = version_parts(version)
    for tag in git("tag", "--list", "v*").splitlines():
        if re.fullmatch("v" + SEMVER, tag) and target <= version_parts(tag[1:]):
            raise ValueError("Version must exceed existing release tags; use Release to retry an immutable tag")


def request_version(sha, before):
    mode = git("ls-tree", sha, "--", REQUEST).split()
    if not mode or mode[0] != "100644":
        raise ValueError("Release request must be a regular tracked JSON file")
    def unique_keys(pairs):
        request = dict(pairs)
        if len(request) != len(pairs):
            raise ValueError("Release request contains duplicate JSON keys")
        return request

    request = json.loads(Path(REQUEST).read_text(), object_pairs_hook=unique_keys)
    if not isinstance(request, dict) or set(request) != {"version", "source_sha"}:
        raise ValueError("Release request requires exactly version and source_sha")
    require_sha(request["source_sha"])
    parents = git("rev-list", "--parents", "-n", "1", sha).split()
    if parents != [sha, request["source_sha"]] or before != request["source_sha"]:
        raise ValueError("Request must be a single-parent commit directly after event.before/source_sha")
    changed = git("diff-tree", "--no-commit-id", "--name-only", "-r", sha).splitlines()
    if changed != [REQUEST]:
        raise ValueError("Activation commit must change only .github/release-request.json")
    version_parts(request["version"])
    return request["version"]


def plan(event, ref, branch, sha, before="", bump="current", explicit=""):
    if ref != f"refs/heads/{branch}":
        raise ValueError("Releases can only be prepared from the default branch")
    require_checkout(sha, branch)
    if event == "push":
        target = select_version(current_version(), "custom", request_version(sha, before))
    elif event == "workflow_dispatch":
        target = select_version(current_version(), bump, explicit)
    else:
        raise ValueError("Release preparation requires manual dispatch or an explicit request commit")
    require_new_tag(target)
    return {"version": target, "tag": f"v{target}", "base_sha": sha, "branch": branch}


def replace_version(text, old, new):
    pattern = r'(?m)^(version\s*=\s*["\'])' + re.escape(old) + r'(["\'][^\n]*)$'
    changed, count = re.subn(pattern, lambda m: m[1] + new + m[2], text)
    if count != 1:
        raise ValueError("Expected one matching version field")
    return changed


def rewrite_section(text, section, transform):
    pattern = r"(?ms)(^\[" + re.escape(section) + r"\][^\n]*\n)(.*?)(?=^\[|\Z)"
    changed, count = re.subn(pattern, lambda m: m[1] + transform(m[2]), text)
    if count != 1:
        raise ValueError(f"Expected one [{section}] table")
    return changed


def rewrite_lock(text, old, new):
    packages = tomllib.loads(text)["package"]
    own = [p for p in packages if p["name"] == "zhex"]
    if len(own) != 1 or own[0]["version"] != old or "source" in own[0]:
        raise ValueError("Lockfile must contain exactly the current local zhex version")
    count = 0

    def update(match):
        nonlocal count
        if tomllib.loads(match[0])["package"][0]["name"] != "zhex":
            return match[0]
        count += 1
        return replace_version(match[0], old, new)

    changed = re.sub(r"(?ms)^\[\[package\]\][^\n]*\n.*?(?=^\[\[package\]\]|\Z)", update, text)
    if count != 1:
        raise ValueError("Expected one local zhex lockfile entry")
    return changed


def rewrite_path_dependency(text, old, new):
    dep = tomllib.loads(text)["dependencies"]["zhex"]
    if not isinstance(dep, dict) or dep.get("path") != ".." or dep.get("version") not in (None, old, f"={old}"):
        raise ValueError("Comparison path dependency does not match the current zhex version")

    def table(body):
        def line(match):
            fields = match[1]
            if "version" in dep:
                fields, count = re.subn(r'\bversion\s*=\s*(["\'])[^"\']*\1', f'version = "={new}"', fields)
                if count != 1:
                    raise ValueError("Expected one inline path version")
            else:
                fields = fields.rstrip().rstrip(",") + f', version = "={new}" '
            return "zhex = {" + fields + "}" + match[2]

        changed, count = re.subn(r"(?m)^zhex\s*=\s*\{([^}\n]*)\}([^\n]*)$", line, body)
        if count != 1:
            raise ValueError("Expected one inline zhex path dependency")
        return changed

    changed = rewrite_section(text, "dependencies", table)
    if tomllib.loads(changed)["dependencies"]["zhex"] != dict(dep, version=f"={new}"):
        raise ValueError("Comparison dependency changed beyond its version")
    return changed


def apply_version(target, sha, branch):
    require_checkout(sha, branch)
    old = current_version()
    select_version(old, "custom", target)
    require_new_tag(target)
    original = {name: Path(name).read_text() for name in VERSION_FILES}
    changed = {
        "Cargo.toml": rewrite_section(original["Cargo.toml"], "package", lambda body: replace_version(body, old, target)),
        "Cargo.lock": rewrite_lock(original["Cargo.lock"], old, target),
        "comparison/Cargo.toml": rewrite_path_dependency(original["comparison/Cargo.toml"], old, target),
        "comparison/Cargo.lock": rewrite_lock(original["comparison/Cargo.lock"], old, target),
    }
    # Validate every proposed TOML before writing; other packages and both
    # lockfile format versions remain byte-for-byte unchanged.
    for text in changed.values():
        tomllib.loads(text)
    for name, text in changed.items():
        if text != original[name]:
            Path(name).write_text(text)
    return {"version": target, "tag": f"v{target}"}


def resolve(tag, trusted_ref, expected_sha=""):
    if not tag.startswith("v"):
        raise ValueError("Release tag must be vX.Y.Z")
    version_parts(tag[1:])
    if not trusted_ref.startswith("refs/remotes/origin/"):
        raise ValueError("Trusted release history must be the origin default branch")
    git("check-ref-format", trusted_ref)
    sha = git("rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}")
    if expected_sha:
        require_sha(expected_sha)
        if sha != expected_sha:
            raise ValueError("Release tag changed after validation")
    if subprocess.run(["git", "merge-base", "--is-ancestor", sha, trusted_ref]).returncode != 0:
        raise ValueError("Release commit is outside the trusted default-branch history")
    package = tomllib.loads(git("show", f"{sha}:Cargo.toml"))["package"]
    if package["name"] != "zhex" or tag != f"v{package['version']}":
        raise ValueError("Release tag does not match zhex's Cargo.toml version")
    return {"sha": sha, "tag": tag, "version": package["version"]}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("plan")
    command.add_argument("--event", required=True)
    command.add_argument("--ref", required=True)
    command.add_argument("--branch", required=True)
    command.add_argument("--sha", required=True)
    command.add_argument("--before", default="")
    command.add_argument("--bump", default="current")
    command.add_argument("--version", default="")
    command = commands.add_parser("apply")
    command.add_argument("--version", required=True)
    command.add_argument("--sha", required=True)
    command.add_argument("--branch", required=True)
    command = commands.add_parser("resolve")
    command.add_argument("tag")
    command.add_argument("--expected-sha", default="")
    command.add_argument("--trusted-ref", required=True)
    args = parser.parse_args(argv)
    try:
        if args.command == "plan":
            result = plan(args.event, args.ref, args.branch, args.sha, args.before, args.bump, args.version)
        elif args.command == "apply":
            result = apply_version(args.version, args.sha, args.branch)
        else:
            result = resolve(args.tag, args.trusted_ref, args.expected_sha)
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Release validation failed: {error}\n")
    print(json.dumps(result, sort_keys=True))
    if "GITHUB_OUTPUT" in os.environ:
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
            output.writelines(f"{key}={value}\n" for key, value in result.items())


if __name__ == "__main__":
    main()
