"""Bundle legacy build-whitelisted packages without replacing native Operit2 tools.

Static API-name checks are not parameter, hook, native bridge or device tests.
"""
from __future__ import annotations
import hashlib
import importlib.util
import io
import json
import os
import re
import subprocess
import sys
import zipfile
from pathlib import Path

REVISION = 'legacy-whitelist-js-toolpkg-v3'
METADATA = re.compile(r'/\*\s*METADATA\s*([\s\S]*?)\*/')
NAME = re.compile(r'([\"\']?name[\"\']?\s*:\s*)([\"\'][^\"\']+[\"\']|[A-Za-z0-9_-]+)')
ENABLED = re.compile(r'([\"\']?enabled(?:ByDefault|_by_default)[\"\']?\s*:\s*)(true|false)')
TOOLS = re.compile(r'\bTools\.([A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)+)\s*\(')
PACKAGE_REFERENCE = re.compile(
    r'(?P<prefix>\b(?:[A-Z][A-Z0-9_]*_)?(?:PACKAGE_NAME|TOOLPKG_ID|SUBPACKAGE_ID)\s*=\s*'
    r'|\b(?:packageName|subpackageId|toolPkgId|toolpkgId)\s*:\s*)'
    r'(?P<quote>[\"\x27])(?P<value>[^\"\x27]+)(?P=quote)'
)
LEGACY_HOST_CLASS = re.compile(r'\bcom\.ai\.assistance\.operit\.(?:[A-Za-z_$][\w$]*\.)*[A-Z][\w$]*')


def namespace_references(source: str, package_ids: dict[str, str]) -> str:
    """Update package identifiers in known code contexts, preserving paths and labels."""
    def replace(match):
        value = package_ids.get(match['value'], match['value'])
        return match['prefix'] + match['quote'] + value + match['quote']
    return PACKAGE_REFERENCE.sub(replace, source)


def host_dependencies(source: str) -> dict:
    return {'native_global_dependencies': sorted(set(re.findall(r'\b(Java|Android|NativeInterface)\s*\.', source))),
            'legacy_host_class_dependencies': sorted({name for name in LEGACY_HOST_CLASS.findall(source)
                                                     if not name.rsplit('.', 1)[-1].isupper()})}


def check_javascript(data: bytes, filename: str) -> None:
    # Bundled browser resources have ES import/export declarations; generated
    # ToolPkg host scripts use CommonJS. Make the grammar explicit for stdin.
    # Node's .js autodetection can return success without checking ESM syntax.
    source = data.decode('utf-8-sig')
    module = bool(re.search(r'^\s*(?:export\b|import\b(?!\s*\())', source, re.MULTILINE))
    kind = 'module' if module else 'commonjs'
    checked = subprocess.run(['node', f'--input-type={kind}', '--check', '-'],
                             input=data, capture_output=True)
    if checked.returncode:
        raise RuntimeError(f'Invalid bundled JavaScript {filename}:\n'
                           + checked.stderr.decode('utf-8', errors='replace'))

def known_methods(root: Path) -> set[str]:
    bindings = root/'core/crates/plugin/sdk/src/js_sdk/runtime_bindings.rs'
    return {ns+'.'+method for ns,method in re.findall(r'namespace: "([^"]+)", method: "([^"]+)"',bindings.read_text(encoding='utf-8'))}

def convert(source: str, filename: str, available: set[str]) -> tuple[str,dict]:
    match = METADATA.search(source)
    if not match:
        raise RuntimeError(f'Legacy package has no metadata: {filename}')
    metadata = match.group(1)
    name = NAME.search(metadata)
    if not name:
        raise RuntimeError(f'Legacy package has no name: {filename}')
    original_id = name[2].strip('\"\'')
    package_id = 'legacy_' + original_id
    if not re.fullmatch(r'[A-Za-z0-9_-]+',package_id):
        raise RuntimeError(f'Unsupported legacy package id: {original_id}')
    metadata = NAME.sub(lambda m:m[1]+json.dumps(package_id),metadata,count=1)
    if ENABLED.search(metadata):
        metadata = ENABLED.sub(lambda m:m[1]+'false',metadata)
    else:
        end = metadata.rfind('}')
        trimmed = metadata[:end].rstrip()
        comma = '' if trimmed.endswith((',', '{')) else ','
        metadata = trimmed+comma+'\n"enabledByDefault": false\n'+metadata[end:]
    body = source[:match.start()] + source[match.end():]
    required = sorted(set(TOOLS.findall(body)))
    missing = sorted(set(required)-available)
    host = host_dependencies(body)
    # Shadow namespaces locally; never mutate global Tools or claim unsupported actions succeeded.
    setup = ["const legacyTools = Object.create(baseTools);", "const copied = new Map([['', legacyTools]]);"]
    for api in missing:
        components=api.split('.')
        for i in range(1,len(components)):
            parent='.'.join(components[:i-1]); path='.'.join(components[:i]); key=components[i-1]
            setup.append(f"if (!copied.has({json.dumps(path)})) {{ const parent = copied.get({json.dumps(parent)}); const child = Object.create(parent[{json.dumps(key)}] || null); parent[{json.dumps(key)}] = child; copied.set({json.dumps(path)}, child); }}")
        message=f'Operit2 尚未提供旧版接口 Tools.{api}（工具包 {package_id}）；该操作需要移植，未执行。'
        setup.append(f"copied.get({json.dumps('.'.join(components[:-1]))})[{json.dumps(components[-1])}] = function () {{ throw new Error({json.dumps(message,ensure_ascii=False)}); }};")
    wrapped=('/* METADATA\n'+metadata.strip()+'\n*/\n'
             '// Adapted for the enhanced Operit2 channel; see bundled legacy report and LICENSE.\n'
             '(function (baseTools) {\n'+ '\n'.join(setup)+'\n'
             '(function (Tools) {\n'+body+'\n})(legacyTools);\n})(Tools);\n')
    return wrapped, {'file':filename,'original_id':original_id,'package_id':package_id,
        'enabled_by_default':False,'required_methods':required,'missing_methods':missing,
        **host,
        'status':'requires_host_port' if missing or host['legacy_host_class_dependencies'] else 'static_api_match_only',
        'original_sha256':hashlib.sha256(source.encode()).hexdigest(),
        'packaged_sha256':hashlib.sha256(wrapped.encode()).hexdigest(),'device_tested':False}

def _legacy_sync(legacy: Path):
    spec = importlib.util.spec_from_file_location('operit_legacy_sync', legacy/'tools/example_packages/sync_example_packages.py')
    if spec is None or spec.loader is None:
        raise RuntimeError('Legacy package helper is missing')
    module = importlib.util.module_from_spec(spec); sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _compile_legacy(legacy: Path, folders: list[Path], sha: str) -> None:
    state = legacy/'.operit2-ts-built'
    # A fresh immutable checkout per run; avoid repeating TS in preflight/Web/APK.
    if state.exists() and state.read_text().strip() == sha:
        return
    tsc = 'tsc.cmd' if os.name == 'nt' else 'tsc'
    for config in [legacy/'examples/tsconfig.json', *(d/'tsconfig.json' for d in folders)]:
        subprocess.run([tsc, '-p', str(config), '--pretty', 'false'], cwd=legacy, check=True)
    state.write_text(sha, encoding='ascii')


def _convert_toolpkg(folder: Path, available: set[str], sync, legacy: Path) -> tuple[bytes, dict]:
    manifest = json.loads((folder/'manifest.json').read_text(encoding='utf-8'))
    original_id = manifest['toolpkg_id']
    package_id = 'legacy.' + original_id
    manifest['toolpkg_id'] = package_id
    manifest['enabled_by_default'] = False
    package_ids = {original_id: package_id}
    for subpackage in manifest.get('subpackages', []):
        original_subpackage = subpackage['id']
        subpackage['id'] = 'legacy_' + original_subpackage
        package_ids[original_subpackage] = subpackage['id']
        subpackage['enabled_by_default'] = False
    # Upstream packer includes directory resources and ignored generated dist entries.
    files = sync._iter_files_for_pack(legacy, folder)
    output = io.BytesIO(); required = set(); native_globals = set(); legacy_host_classes = set()
    with zipfile.ZipFile(output, 'w', zipfile.ZIP_DEFLATED) as archive:
        for file in files:
            name = file.relative_to(folder).as_posix()
            data = file.read_bytes()
            if name == 'manifest.json':
                data = (json.dumps(manifest,ensure_ascii=False,indent=2)+'\n').encode()
            elif file.suffix == '.js':
                text = data.decode('utf-8-sig')
                required.update(TOOLS.findall(text))
                host = host_dependencies(text)
                native_globals.update(host['native_global_dependencies'])
                legacy_host_classes.update(host['legacy_host_class_dependencies'])
                # UI modules and IPC helpers address the renamed manifest ids explicitly.
                text = namespace_references(text, package_ids)
                if METADATA.search(text):
                    text,_ = convert(text,name,available)
                data = text.encode()
                check_javascript(data, f'{folder.name}/{name}')
            entry=zipfile.ZipInfo(name,date_time=(2020,1,1,0,0,0))
            entry.compress_type=zipfile.ZIP_DEFLATED
            archive.writestr(entry,data)
    missing=sorted(required-available)
    return output.getvalue(), {'file':folder.name+'.toolpkg','original_id':original_id,
        'package_id':package_id,'enabled_by_default':False,'required_methods':sorted(required),
        'missing_methods':missing,'native_global_dependencies':sorted(native_globals),
        'legacy_host_class_dependencies':sorted(legacy_host_classes),
        'status':'requires_host_port' if missing or legacy_host_classes else 'static_api_match_only',
        'device_tested':False}


def install(root: Path, legacy: Path, sha: str) -> dict:
    sync = _legacy_sync(legacy)
    whitelist=sync._read_whitelist_file(legacy/'tools/example_packages/packages_whitelist.txt')
    plans=[]
    for item in whitelist:
        plan=sync._resolve_plan_item(legacy/'examples',item)
        if plan is None:
            raise RuntimeError(f'Legacy whitelist item has no source: {item}')
        plans.append(plan)
    if not plans:
        raise RuntimeError('Legacy build whitelist is empty')
    _compile_legacy(legacy,[p.source for p in plans if p.mode=='pack'],sha)
    available=known_methods(root)
    dest=root/'plugins/packages/buildin'
    report_dir=root/'tools/release/dist/compatibility/legacy-operit'
    report_dir.mkdir(parents=True,exist_ok=True)
    planned={}
    for plan in plans:
        if plan.mode == 'pack':
            data,info=_convert_toolpkg(plan.source,available,sync,legacy)
            output='legacy_'+plan.source.name+'.toolpkg'
        else:
            if plan.source.suffix != '.js':
                raise RuntimeError(f'New legacy package format needs explicit import handling: {plan.source}')
            text,info=convert(plan.source.read_text(encoding='utf-8-sig'),plan.source.name,available)
            data=text.encode(); output=info['package_id']+'.js'
        if output in planned:
            raise RuntimeError(f'Duplicate legacy package output: {output}')
        info['output']=output; info['packaged_sha256']=hashlib.sha256(data).hexdigest()
        planned[output]=(data,info)
    old_report=report_dir/'manifest.json'
    previous=json.loads(old_report.read_text(encoding='utf-8')) if old_report.exists() else {}
    owned={p.get('output',p['package_id']+'.js') for p in previous.get('packages',[])}
    for name in planned:
        if (dest/name).exists() and name not in owned:
            raise RuntimeError(f'Refusing to overwrite an upstream package: {name}')
    for name in owned - planned.keys():
        if name.startswith('legacy_') and '/' not in name and '\\' not in name:
            (dest/name).unlink(missing_ok=True)
    for name,(data,info) in planned.items():
        (dest/name).write_bytes(data)
        if name.endswith('.js'):
            subprocess.run(['node','--check',str(dest/name)],check=True,capture_output=True)
    result={'revision':REVISION,'source_repository':'AAswordman/Operit','source_commit':sha,
            'source_directory':'examples (official build whitelist)', 'count':len(planned),
            'packages':[item[1] for item in planned.values()], 'device_tested':False}
    old_report.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    (report_dir/'UPSTREAM-LICENSE').write_bytes((legacy/'LICENSE').read_bytes())
    (report_dir/'README.txt').write_text('Legacy packages are built from the official whitelist, namespaced and disabled by default. API names matching does not establish parameter/result, ToolPkg hook, Java/Android bridge compatibility or device behavior. Missing host API dependencies are listed in the report; no privileged or platform-specific API is silently emulated. Original sources and license are in Operit-legacy-source.zip. Do not enable a legacy hook plugin and the equivalent native plugin simultaneously.\n',encoding='utf-8')
    print(f'Legacy whitelist: {len(planned)} packages bundled, including ToolPkg resources.',flush=True)
    return result
