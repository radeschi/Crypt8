#!/usr/bin/env python3
"""Rename GitHub Release assets to orangeEncrypt-<version>-<os>-<arch>.<ext>.

The minisign signature covers file bytes, not the asset name. latest.json
stores the GitHub asset id, so renaming the asset leaves the updater URL valid.
"""

from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request

EXTENSIONS = (
    ".app.tar.gz",
    ".AppImage",
    ".dmg",
    ".deb",
    ".rpm",
    ".msi",
    ".exe",
)

OS_BY_EXT = {
    ".app.tar.gz": "macos",
    ".dmg": "macos",
    ".AppImage": "linux",
    ".deb": "linux",
    ".rpm": "linux",
    ".msi": "windows",
    ".exe": "windows",
}


def public_name(asset_name: str, product: str, version: str) -> str | None:
    if asset_name == "latest.json" or not asset_name.startswith(product):
        return None
    signature = asset_name.endswith(".sig")
    base = asset_name[: -len(".sig")] if signature else asset_name
    extension = next((item for item in EXTENSIONS if base.endswith(item)), None)
    if extension is None or version not in base:
        return None
    arch = arch_label(base)
    if arch is None:
        return None
    renamed = f"{product}-{version}-{OS_BY_EXT[extension]}-{arch}{extension}"
    if signature:
        renamed += ".sig"
    return renamed


def arch_label(name: str) -> str | None:
    folded = name.lower()
    if "aarch64" in folded or "arm64" in folded:
        return "arm64"
    if "x86_64" in folded or "amd64" in folded or "x64" in folded:
        return "x64"
    return None


def matches_job(renamed: str, release_os: str, release_arch: str) -> bool:
    return f"-{release_os}-{release_arch}." in renamed or renamed.endswith(
        f"-{release_os}-{release_arch}"
    )


def rename_assets() -> None:
    product = os.environ.get("PRODUCT_NAME", "orangeEncrypt")
    version = os.environ["APP_VERSION"]
    release_os = os.environ["RELEASE_OS"]
    release_arch = os.environ["RELEASE_ARCH"]
    repository = os.environ["GITHUB_REPOSITORY"]
    token = os.environ["GITHUB_TOKEN"]
    tag = os.environ.get("RELEASE_TAG", f"v{version}")

    release = github_json(
        token,
        f"https://api.github.com/repos/{repository}/releases/tags/{tag}",
    )
    for asset in release["assets"]:
        current = asset["name"]
        renamed = public_name(current, product, version)
        if renamed is None or renamed == current or not matches_job(renamed, release_os, release_arch):
            continue
        print(f"{current} -> {renamed}")
        github_json(
            token,
            f"https://api.github.com/repos/{repository}/releases/assets/{asset['id']}",
            method="PATCH",
            body={"name": renamed, "label": renamed},
        )


def github_json(token: str, url: str, method: str = "GET", body: dict | None = None) -> dict:
    data = None if body is None else json.dumps(body).encode()
    request = urllib.request.Request(
        url,
        data=data,
        method=method,
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "X-GitHub-Api-Version": "2022-11-28",
            "Content-Type": "application/json",
            "User-Agent": "orangeencrypt-release-rename",
        },
    )
    try:
        with urllib.request.urlopen(request) as response:
            payload = response.read().decode()
    except urllib.error.HTTPError as error:
        detail = error.read().decode()
        raise SystemExit(f"GitHub API {error.code} for {url}: {detail}") from error
    return json.loads(payload) if payload else {}


if __name__ == "__main__":
    rename_assets()
