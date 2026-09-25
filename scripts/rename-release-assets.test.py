import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "rename_release_assets",
    Path(__file__).with_name("rename-release-assets.py"),
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

product = "orangeEncrypt"
version = "1.0.5"
expected = {
    "orangeEncrypt_1.0.5_aarch64.dmg": "orangeEncrypt-1.0.5-macos-arm64.dmg",
    "orangeEncrypt_1.0.5_x64.dmg": "orangeEncrypt-1.0.5-macos-x64.dmg",
    "orangeEncrypt_1.0.5_aarch64.app.tar.gz": "orangeEncrypt-1.0.5-macos-arm64.app.tar.gz",
    "orangeEncrypt_1.0.5_x86_64.app.tar.gz": "orangeEncrypt-1.0.5-macos-x64.app.tar.gz",
    "orangeEncrypt_1.0.5_aarch64.app.tar.gz.sig": "orangeEncrypt-1.0.5-macos-arm64.app.tar.gz.sig",
    "orangeEncrypt_1.0.5_aarch64.AppImage": "orangeEncrypt-1.0.5-linux-arm64.AppImage",
    "orangeEncrypt_1.0.5_amd64.AppImage": "orangeEncrypt-1.0.5-linux-x64.AppImage",
    "orangeEncrypt_1.0.5_aarch64.AppImage.sig": "orangeEncrypt-1.0.5-linux-arm64.AppImage.sig",
    "orangeEncrypt_1.0.5_arm64.deb": "orangeEncrypt-1.0.5-linux-arm64.deb",
    "orangeEncrypt_1.0.5_amd64.deb": "orangeEncrypt-1.0.5-linux-x64.deb",
    "orangeEncrypt_1.0.5_amd64.deb.sig": "orangeEncrypt-1.0.5-linux-x64.deb.sig",
    "orangeEncrypt-1.0.5-1.aarch64.rpm": "orangeEncrypt-1.0.5-linux-arm64.rpm",
    "orangeEncrypt-1.0.5-1.x86_64.rpm": "orangeEncrypt-1.0.5-linux-x64.rpm",
    "orangeEncrypt-1.0.5-1.x86_64.rpm.sig": "orangeEncrypt-1.0.5-linux-x64.rpm.sig",
    "orangeEncrypt_1.0.5_arm64_en-US.msi": "orangeEncrypt-1.0.5-windows-arm64.msi",
    "orangeEncrypt_1.0.5_x64_en-US.msi": "orangeEncrypt-1.0.5-windows-x64.msi",
    "orangeEncrypt_1.0.5_x64_en-US.msi.sig": "orangeEncrypt-1.0.5-windows-x64.msi.sig",
    "orangeEncrypt_1.0.5_arm64-setup.exe": "orangeEncrypt-1.0.5-windows-arm64.exe",
    "orangeEncrypt_1.0.5_x64-setup.exe": "orangeEncrypt-1.0.5-windows-x64.exe",
    "orangeEncrypt_1.0.5_x64-setup.exe.sig": "orangeEncrypt-1.0.5-windows-x64.exe.sig",
    "latest.json": None,
}

for current, renamed in expected.items():
    got = module.public_name(current, product, version)
    if got != renamed:
        raise SystemExit(f"{current}: expected {renamed}, got {got}")

future = module.public_name("orangeEncrypt_9.2.0_amd64.AppImage", product, "9.2.0")
if future != "orangeEncrypt-9.2.0-linux-x64.AppImage":
    raise SystemExit(future)

print("ok", len(expected))
