import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "rename_release_assets",
    Path(__file__).with_name("rename-release-assets.py"),
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

product = "Crypt8"
version = "1.0.5"
expected = {
    "Crypt8_1.0.5_aarch64.dmg": "Crypt8-1.0.5-macos-arm64.dmg",
    "Crypt8_1.0.5_x64.dmg": "Crypt8-1.0.5-macos-x64.dmg",
    "Crypt8_1.0.5_aarch64.app.tar.gz": "Crypt8-1.0.5-macos-arm64.app.tar.gz",
    "Crypt8_1.0.5_x86_64.app.tar.gz": "Crypt8-1.0.5-macos-x64.app.tar.gz",
    "Crypt8_1.0.5_aarch64.app.tar.gz.sig": "Crypt8-1.0.5-macos-arm64.app.tar.gz.sig",
    "Crypt8_1.0.5_aarch64.AppImage": "Crypt8-1.0.5-linux-arm64.AppImage",
    "Crypt8_1.0.5_amd64.AppImage": "Crypt8-1.0.5-linux-x64.AppImage",
    "Crypt8_1.0.5_aarch64.AppImage.sig": "Crypt8-1.0.5-linux-arm64.AppImage.sig",
    "Crypt8_1.0.5_arm64.deb": "Crypt8-1.0.5-linux-arm64.deb",
    "Crypt8_1.0.5_amd64.deb": "Crypt8-1.0.5-linux-x64.deb",
    "Crypt8_1.0.5_amd64.deb.sig": "Crypt8-1.0.5-linux-x64.deb.sig",
    "Crypt8-1.0.5-1.aarch64.rpm": "Crypt8-1.0.5-linux-arm64.rpm",
    "Crypt8-1.0.5-1.x86_64.rpm": "Crypt8-1.0.5-linux-x64.rpm",
    "Crypt8-1.0.5-1.x86_64.rpm.sig": "Crypt8-1.0.5-linux-x64.rpm.sig",
    "Crypt8_1.0.5_arm64_en-US.msi": "Crypt8-1.0.5-windows-arm64.msi",
    "Crypt8_1.0.5_x64_en-US.msi": "Crypt8-1.0.5-windows-x64.msi",
    "Crypt8_1.0.5_x64_en-US.msi.sig": "Crypt8-1.0.5-windows-x64.msi.sig",
    "Crypt8_1.0.5_arm64-setup.exe": "Crypt8-1.0.5-windows-arm64.exe",
    "Crypt8_1.0.5_x64-setup.exe": "Crypt8-1.0.5-windows-x64.exe",
    "Crypt8_1.0.5_x64-setup.exe.sig": "Crypt8-1.0.5-windows-x64.exe.sig",
    "latest.json": None,
}

for current, renamed in expected.items():
    got = module.public_name(current, product, version)
    if got != renamed:
        raise SystemExit(f"{current}: expected {renamed}, got {got}")

future = module.public_name("Crypt8_9.2.0_amd64.AppImage", product, "9.2.0")
if future != "Crypt8-9.2.0-linux-x64.AppImage":
    raise SystemExit(future)

print("ok", len(expected))
