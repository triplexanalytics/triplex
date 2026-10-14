#!/usr/bin/env python3
"""Compute (and optionally create) the per-package version tags for HEAD.

Every commit on main that touches a package bumps that package: a `feat` or breaking (`!`)
subject bumps the minor, anything else the patch. Tags are
`<package>-v<major>.<minor>.<patch>` and are the only source of truth for a
version; no file in the repo carries one.

For each package this walks the first-parent commits since the package's
latest reachable tag (or since the root commit when it has none) and tags the last commit that touched it.

    scripts/version_tags.py            # print what would be tagged
    scripts/version_tags.py --apply    # create the tags locally
    scripts/version_tags.py --badges <dir>   # write one <package>.svg per package
    scripts/version_tags.py --self-test
"""

import pathlib
import re
import subprocess
import sys

# First match wins, so the UI under apps/triplex-web/www/ is not counted as
# part of the Rust workspace that otherwise owns apps/.
PACKAGES = [
    ("web", ("apps/triplex-web/www/",)),
    ("python", ("packages/triplex-python/",)),
    ("jupyter", ("packages/triplex-jupyter/",)),
    ("workspace", ("crates/", "apps/", "Cargo.toml", "Cargo.lock")),
]

# Label on each package's README badge.
BADGE_LABELS = {
    "workspace": "engine + CLI",
    "web": "web UI",
    "python": "Python",
    "jupyter": "Jupyter widget",
}

SUBJECT = re.compile(r"^(\w+)(\([^)]*\))?(!)?:")


def package_of(path):
    for name, prefixes in PACKAGES:
        if path.startswith(prefixes):
            return name
    return None


def bump(version, subject):
    major, minor, patch = version
    m = SUBJECT.match(subject)
    if m and (m.group(1) == "feat" or m.group(3)):
        return (major, minor + 1, 0)
    return (major, minor, patch + 1)


def git(*args):
    return subprocess.run(
        ["git", *args], check=True, capture_output=True, text=True
    ).stdout.strip()


def latest_tag(package):
    result = subprocess.run(
        ["git", "describe", "--tags", "--abbrev=0", "--match", f"{package}-v*", "HEAD"],
        capture_output=True,
        text=True,
    )
    return result.stdout.strip() if result.returncode == 0 else None


def plan():
    """Return {package: (version, commit)} for every package that needs a new tag."""
    planned = {}
    for package, _ in PACKAGES:
        tag = latest_tag(package)
        version = tuple(int(n) for n in tag.split("-v")[1].split(".")) if tag else (0, 0, 0)
        commits = git(
            "rev-list", "--first-parent", "--reverse", f"{tag}..HEAD" if tag else "HEAD"
        ).split()
        for commit in commits:
            files = git(
                "diff-tree", "--root", "--no-commit-id", "--name-only", "-r", "-m",
                "--first-parent", commit,
            ).splitlines()
            if any(package_of(f) == package for f in files):
                version = bump(version, git("log", "-1", "--format=%s", commit))
                planned[package] = (version, commit)
    return planned


def badge_svg(label, value):
    """A flat two-part badge, drawn here so the README needs no badge service."""
    # ponytail: 7px per character is an estimate, not real text metrics; fine
    # for short labels and version numbers.
    lw, vw = 7 * len(label) + 12, 7 * len(value) + 12
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{lw + vw}" height="20" '
        f'role="img" aria-label="{label}: {value}">'
        f'<rect width="{lw}" height="20" rx="3" fill="#555"/>'
        f'<rect x="{lw}" width="{vw}" height="20" rx="3" fill="#007ec6"/>'
        f'<rect x="{lw}" width="4" height="20" fill="#007ec6"/>'
        '<g fill="#fff" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" '
        'font-size="11" text-anchor="middle">'
        f'<text x="{lw / 2}" y="14">{label}</text>'
        f'<text x="{lw + vw / 2}" y="14">{value}</text></g></svg>\n'
    )


def write_badges(directory):
    out = pathlib.Path(directory)
    out.mkdir(parents=True, exist_ok=True)
    for package, _ in PACKAGES:
        tag = latest_tag(package)
        value = "v" + tag.split("-v")[1] if tag else "untagged"
        (out / f"{package}.svg").write_text(badge_svg(BADGE_LABELS[package], value))


def self_test():
    assert package_of("apps/triplex-web/www/src/App.jsx") == "web"
    assert package_of("apps/triplex-web/src/lib.rs") == "workspace"
    assert package_of("crates/triplex-core/src/lib.rs") == "workspace"
    assert package_of("README.md") is None
    assert package_of("packages/triplex-python/src/lib.rs") == "python"
    assert package_of("packages/triplex-jupyter/pyproject.toml") == "jupyter"
    assert bump((0, 3, 2), "feat(web): compare dialog") == (0, 4, 0)
    assert bump((0, 3, 2), "fix(web): disable the tier picker") == (0, 3, 3)
    assert bump((0, 3, 2), "refactor(io)!: rename the sink trait") == (0, 4, 0)
    assert bump((0, 3, 2), "Merge something without a prefix") == (0, 3, 3)
    assert 'aria-label="web UI: v0.30.0"' in badge_svg("web UI", "v0.30.0")
    print("ok")


def main():
    if "--self-test" in sys.argv:
        return self_test()
    if "--badges" in sys.argv:
        return write_badges(sys.argv[sys.argv.index("--badges") + 1])
    for package, (version, commit) in plan().items():
        tag = f"{package}-v{'.'.join(map(str, version))}"
        print(tag, commit[:9])
        if "--apply" in sys.argv:
            git("tag", tag, commit)


if __name__ == "__main__":
    main()
