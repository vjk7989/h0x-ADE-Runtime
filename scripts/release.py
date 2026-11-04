"""Validate a stable release version against its source; never build, tag, push, or publish."""

import argparse
import re
import subprocess
import tomllib


STABLE_VERSION = r"[0-9]+\.[0-9]+\.[0-9]+"


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], text=True).strip()


def stable_version(version: str) -> str:
    if not re.fullmatch(STABLE_VERSION, version):
        raise ValueError(f"invalid stable version: {version}")
    return version


def resolve(ref: str) -> str:
    return git("rev-parse", "--verify", f"{ref}^{{commit}}")


def cargo_version(text: str, path: str) -> str:
    data = tomllib.loads(text)
    if path == "Cargo.toml":
        return data["package"]["version"]
    packages = [p for p in data["package"] if p["name"] == "h0x" and "source" not in p]
    if len(packages) != 1:
        raise ValueError("expected exactly one local h0x package in Cargo.lock")
    return packages[0]["version"]


def validate_version(commit: str, version: str) -> str:
    stable_version(version)
    commit = resolve(commit)
    versions = [cargo_version(git("show", f"{commit}:{path}"), path) for path in ("Cargo.toml", "Cargo.lock")]
    if versions[0] != versions[1] or versions[0] != version:
        raise ValueError("release version must match Cargo.toml and Cargo.lock")
    return commit


def validate_tag(tag: str) -> str:
    if not re.fullmatch(f"v{STABLE_VERSION}", tag):
        raise ValueError("expected a stable vX.Y.Z tag")
    return validate_version(f"refs/tags/{tag}", tag[1:])


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    check = commands.add_parser("check")
    check.add_argument("--version", required=True)
    check.add_argument("--commit", default="HEAD")
    tag = commands.add_parser("check-tag")
    tag.add_argument("--tag", required=True)
    args = parser.parse_args()
    if args.command == "check":
        commit = validate_version(args.commit, args.version)
        print(f"v{args.version} matches {commit}")
    else:
        commit = validate_tag(args.tag)
        print(f"{args.tag} releases {commit}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, subprocess.CalledProcessError, KeyError) as error:
        raise SystemExit(f"error: {error}") from error
