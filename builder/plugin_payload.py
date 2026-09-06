"""Source fixes (enhanced only) and checks for Rust-embedded tool packages.

Packages are include_bytes! data in liboperit_flutter_bridge.so, not APK assets.
This verifies packaging, not installation, permissions, or device behavior.
"""
from __future__ import annotations
import difflib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import re
import sys
import zipfile

FIX_REVISION = "extended-chat-timer-errors-v1"
PLUGIN_SOURCE = "plugins/packages/buildin/extended_chat.ts"
NATIVE_ENTRY = "lib/arm64-v8a/liboperit_flutter_bridge.so"
TIMER_PATTERN = re.compile(
    r"(?m)^(?P<i>[ \t]*)const timeoutPromise = new Promise(?P<t><null>)?\(\(resolve\) => \{\s*"
    r"setTimeout\(\(\) => resolve\(null\), timeoutMs\);\s*\}\);\s*"
    r"const sendResult = await Promise\.race\(\[sendPromise, timeoutPromise\]\);"
)
OLD_ERROR = "message: `读取对话消息失败: ${message}`,"
NEW_ERROR = "message: `工具 ${func.name.replace(/_impl$/, '')} 执行失败: ${message}`,"

def fix_source(text: str) -> tuple[str, dict[str, str]]:
    newline = "\r\n" if "\r\n" in text else "\n"
    text = text.replace("\r\n", "\n")
    def timer(match: re.Match) -> str:
        indent, generic = match["i"], match["t"] or ""
        annotation = ": ReturnType<typeof setTimeout> | undefined" if generic else ""
        lines = [
            "// operit2-personal: release the agent timeout timer on every outcome.",
            f"let timeoutId{annotation};",
            f"const timeoutPromise = new Promise{generic}((resolve) => {{",
            "    timeoutId = setTimeout(() => resolve(null), timeoutMs);", "});",
            "const sendResult = await Promise.race([sendPromise, timeoutPromise]).finally(() => {",
            "    if (timeoutId !== undefined) clearTimeout(timeoutId);", "});",
        ]
        return "\n".join(indent + line for line in lines)
    text, count = TIMER_PATTERN.subn(timer, text)
    timer_status = "applied" if count else (
        "already_applied" if "// operit2-personal: release the agent timeout timer" in text
        else "upstream_changed_not_modified")
    error_count = text.count(OLD_ERROR)
    text = text.replace(OLD_ERROR, NEW_ERROR)
    error_status = "applied" if error_count else (
        "already_applied" if NEW_ERROR in text else "upstream_changed_not_modified")
    return text.replace("\n", newline), {"timer_cleanup": timer_status, "error_label": error_status}

def apply_source_fixes(root: Path) -> dict:
    source = root / PLUGIN_SOURCE
    folder = root / "tools/release/dist/compatibility/extended_chat"
    folder.mkdir(parents=True, exist_ok=True)
    manifest_file = folder / "manifest.json"
    if not source.is_file():
        result = {"revision": FIX_REVISION, "source": PLUGIN_SOURCE,
                  "status": "upstream_source_moved_not_modified"}
    else:
        before = source.read_bytes()
        after_text, statuses = fix_source(before.decode("utf-8"))
        after = after_text.encode("utf-8")
        if before == after and manifest_file.is_file():
            previous = json.loads(manifest_file.read_text(encoding="utf-8"))
            if previous.get("after_sha256") == hashlib.sha256(after).hexdigest():
                return previous
        result = {"revision": FIX_REVISION, "source": PLUGIN_SOURCE, "fixes": statuses,
                  "before_sha256": hashlib.sha256(before).hexdigest(),
                  "after_sha256": hashlib.sha256(after).hexdigest()}
        if before != after:
            (folder / "extended_chat.ts.before").write_bytes(before)
            (folder / "extended_chat.ts").write_bytes(after)
            diff = "".join(difflib.unified_diff(
                before.decode().splitlines(keepends=True), after_text.splitlines(keepends=True),
                fromfile="a/" + PLUGIN_SOURCE, tofile="b/" + PLUGIN_SOURCE))
            (folder / "source.patch").write_text(diff, encoding="utf-8")
            source.write_bytes(after)
    manifest_file.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print("Plugin source fixes: " + json.dumps(result, ensure_ascii=False), flush=True)
    return result

def load_sync(root: Path):
    path = root / "plugins/tools/sync_plugin_packages.py"
    spec = importlib.util.spec_from_file_location("operit_payload_sync", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Cannot load upstream plugin sync helper: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module

def generated_plugins(root: Path) -> list[tuple[str, str, Path]]:
    sync = load_sync(root)
    result = []
    for group in ("buildin", "external"):
        source = root / "plugins/packages" / group
        if not source.is_dir():
            raise RuntimeError(f"Upstream plugin source directory is missing: {source}")
        plans = sync._collect_sync_plan(source)
        for name in sorted({plan.destination_name for plan in plans}):
            path = root / "core/crates/runtime/application/assets/plugins" / group / name
            if not path.is_file() or path.stat().st_size == 0:
                raise RuntimeError(f"Plugin sync produced no usable output: {group}/{name}")
            result.append((group, name, path))
    if not any(group == "buildin" for group, _, _ in result):
        raise RuntimeError("No built-in packages were planned; check the upstream plugin layout")
    return result

def inspect_toolpkg(data: bytes, name: str) -> dict:
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        bad = archive.testzip()
        if bad:
            raise RuntimeError(f"Corrupt ToolPkg {name}: {bad}")
        names = set(archive.namelist())
        if "manifest.hjson" in names:
            return {"manifest": "manifest.hjson", "entry_check": "upstream_hjson_loader"}
        if "manifest.json" not in names:
            raise RuntimeError(f"Missing ToolPkg manifest: {name}")
        manifest = json.loads(archive.read("manifest.json").decode("utf-8-sig"))
        entries = [manifest.get("main")]
        entries.extend(item.get("entry") for item in manifest.get("subpackages", []))
        for entry in filter(None, entries):
            key = entry.removeprefix("./")
            if key not in names or not archive.getinfo(key).file_size:
                raise RuntimeError(f"Missing/empty ToolPkg entry in {name}: {entry}")
        for resource in manifest.get("resources", []):
            key = resource["path"].removeprefix("./").rstrip("/")
            if key in names and archive.getinfo(key).file_size:
                continue
            if any(n.startswith(key + "/") and archive.getinfo(n).file_size for n in names):
                continue
            raise RuntimeError(f"Missing/empty ToolPkg resource in {name}: {key}")
        return {"manifest": "manifest.json", "package_id": manifest.get("toolpkg_id"),
                "entry_check": "passed", "checked_entries": len(list(filter(None, entries)))}

def inspect_generated(root: Path, native: bytes | None = None) -> dict:
    packages = []
    for group, name, path in generated_plugins(root):
        data = path.read_bytes()
        item = {"group": group, "name": name, "bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest()}
        if name.endswith(".toolpkg"):
            item.update(inspect_toolpkg(data, name))
        if native is not None:
            offset = native.find(data)
            if offset < 0:
                raise RuntimeError(f"APK native library lacks the current plugin bytes: {group}/{name}")
            item["native_offset"] = offset
        packages.append(item)
    return {"schema": 1, "native_entry": NATIVE_ENTRY, "packages": packages,
            "count": len(packages), "native_bytes_verified": native is not None,
            "device_tested": False}

def verify_apk_plugins(root: Path, apk: Path) -> dict:
    with zipfile.ZipFile(apk) as archive:
        report = inspect_generated(root, archive.read(NATIVE_ENTRY))
    folder = root / "tools/release/dist/compatibility/plugin-payload"
    folder.mkdir(parents=True, exist_ok=True)
    with apk.open("rb") as stream:
        report["apk_sha256"] = hashlib.file_digest(stream, "sha256").hexdigest()
    (folder / "manifest.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Verified complete native payloads of {report['count']} plugins (not merely their names).", flush=True)
    return report
