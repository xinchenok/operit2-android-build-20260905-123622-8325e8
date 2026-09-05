"""Build upstream Operit2 for ARM64 with its existing Android build helpers.

Run from the upstream repository root on the Windows GitHub Actions runner.
This script has not been integration-tested in the current chat environment.
"""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path


# The actual entry point intentionally imports only public helpers present in the
# inspected upstream source. It does not modify the application's source code.
def build() -> None:
    root = Path.cwd()
    sys.path.insert(0, str(root / "tools" / "build_scripts"))
    from common import (  # type: ignore[import-not-found]
        DIST_DIR,
        FLUTTER_APP_DIR,
        copy_required_file,
        flutter_command,
        flutter_pub_get,
        prepare_web_access_embedded_assets,
        run,
        staged_non_ohos_flutter_dependencies,
    )
    from build_flutter_android import (  # type: ignore[import-not-found]
        configure_android_flutter_sdk,
        ensure_android_signing,
    )

    prepare_web_access_embedded_assets()
    ensure_android_signing()
    flutter = flutter_command()
    configure_android_flutter_sdk(flutter)
    with staged_non_ohos_flutter_dependencies():
        flutter_pub_get()
        flutter_pub_get(enforce_lockfile=True)
        run(
            [flutter, "build", "apk", "--release", "--no-pub",
             "--target-platform", "android-arm64", "--split-per-abi"],
            cwd=FLUTTER_APP_DIR,
        )

    apk = FLUTTER_APP_DIR / "build/app/outputs/flutter-apk/app-arm64-v8a-release.apk"
    copy_required_file(apk, DIST_DIR / "operit2-android-arm64-personal-test.apk")
    sha = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], text=True, encoding="utf-8"
    ).strip()
    info = {
        "upstream_repository": "https://github.com/AAswordman/Operit2",
        "source_commit": sha,
        "source_url": f"https://github.com/AAswordman/Operit2/tree/{sha}",
        "abi": "arm64-v8a",
        "package_id": "app.operit",
        "official_build": False,
        "device_tested": False,
        "signing": "Personal test key, NOT the upstream author's signing key.",
        "update_warning": "Without supplied signing secrets, each run uses a new key. "
                          "Back up app data before uninstalling a differently signed build.",
    }
    (DIST_DIR / "BUILD-INFO.json").write_text(
        json.dumps(info, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    wrapper = Path(__file__).resolve().parent.parent
    shutil.copy2(wrapper / ".github/workflows/operit2-android-test.yml", DIST_DIR / "build-workflow.yml")
    shutil.copy2(__file__, DIST_DIR / "build_arm64.py")
    shutil.copy2(wrapper / "builder/verify_apk.py", DIST_DIR / "verify_apk.py")
    shutil.copy2(root / "LICENSE", DIST_DIR / "UPSTREAM-LICENSE")


if __name__ == "__main__":
    build()
