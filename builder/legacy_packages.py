"""Build namespaced Operit1 packages with enhanced-only host adapters."""
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

REVISION = 'legacy-host-adapters-v5'
ENHANCEMENTS = Path(__file__).resolve().parents[1] / 'enhancements'
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
MENU_IDENTIFIERS = {
    'com.operit.thinking_guidance': {
        ('TOGGLE_ID', 'thinking_guidance'): 'legacy_thinking_guidance',
        ('ENV_KEY', 'OPERIT_THINKING_GUIDANCE_ENABLED'): 'LEGACY_OPERIT_THINKING_GUIDANCE_ENABLED',
    },
    'com.operit.context_limiter_c': {
        ('limiter', 'ctx_limiter_toggle'): 'legacy_ctx_limiter_toggle',
        ('adjust', 'ctx_limiter_adjust'): 'legacy_ctx_limiter_adjust',
    },
    'com.operit.message_insert_bundle': {
        ('id', 'message_extra_info_injection'): 'legacy_message_extra_info_injection',
    },
}


def namespace_input_menu(source: str, package_id: str) -> str:
    # Flutter routes clicks by toggle id alone. Distinct container ids do not
    # prevent a legacy toggle from dispatching to the native package's hook.
    replacements = MENU_IDENTIFIERS.get(package_id, {})
    pattern = r'\b(?P<field>TOGGLE_ID|ENV_KEY|limiter|adjust|id)(?P<assign>\s*[:=]\s*)(?P<quote>[\"\x27])(?P<value>[^\"\x27]+)(?P=quote)'
    def replace(match):
        value = replacements.get((match['field'], match['value']), match['value'])
        return match['field'] + match['assign'] + match['quote'] + value + match['quote']
    return re.sub(pattern, replace, source)


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


def adapter_methods() -> set[str]:
    contracts = json.loads((ENHANCEMENTS/'legacy-tools/contracts.json').read_text(encoding='utf-8'))
    return set(contracts['methods'])


def check_native_contracts(classes: list[str] | set[str], package_id: str) -> None:
    contracts = json.loads((ENHANCEMENTS/'legacy-native/native-contracts.json').read_text(encoding='utf-8'))
    missing = sorted(set(classes)-set(contracts['supported_legacy_classes']))
    if missing:
        raise RuntimeError(f'Legacy package {package_id} still references unported Operit1 classes: {missing}')


def wrap_host_script(source: str) -> str:
    """Keep each package's compatibility bindings local, including ToolPkg helpers."""
    if not re.search(r'\bTools\b', source):
        return source
    if re.search(r'^\s*(?:export\b|import\b(?!\s*\())', source, re.MULTILINE):
        # Web UI modules use their explicit IPC bridge; they have no host Tools.
        return source
    factory = (ENHANCEMENTS/'legacy-tools/compat.js').read_text(encoding='utf-8')
    return ('(function (baseTools) {\n' + factory + '\n'
            '(function (Tools) {\n' + source + '\n'
            '})(__operitCreateLegacyTools(baseTools));\n})(Tools);\n')


def _prepare_legacy_sources(legacy: Path, sha: str) -> str:
    """Apply reviewed source ports before tsc; include their bytes in its cache key."""
    digest = hashlib.sha256((sha + REVISION).encode())
    staged = {}
    for family in ('legacy-native', 'legacy-tools'):
        base = ENHANCEMENTS/family
        overrides = base/'source-overrides'
        if not overrides.exists():
            continue
        expected = json.loads((base/'expected-source-sha256.json').read_text(encoding='utf-8'))
        for source in sorted(overrides.rglob('*')):
            if not source.is_file():
                continue
            relative = source.relative_to(overrides).as_posix()
            target = legacy/relative
            # Git may check these reviewed text ports out as CRLF on Windows.
            # Manifest hashes describe canonical Git/LF contents, as does tsc input.
            data = source.read_bytes().replace(b'\r\n', b'\n')
            source_hash = hashlib.sha256(data).hexdigest()
            old_hash = hashlib.sha256(target.read_bytes().replace(b'\r\n', b'\n')).hexdigest() if target.exists() else None
            record = expected.get(relative)
            accepted = record.get('source_sha256') if isinstance(record, dict) else record
            if isinstance(record, dict) and record.get('patched_sha256') != source_hash:
                raise RuntimeError(f'Enhanced source adapter digest is stale: {relative}')
            if relative not in expected or old_hash not in (accepted, source_hash):
                raise RuntimeError(f'Operit1 source changed; review enhanced adapter before replacing {relative}')
            digest.update(relative.encode()); digest.update(data)
            staged[target] = data
    patches_path = ENHANCEMENTS/'legacy-tools/package-patches.json'
    patches = json.loads(patches_path.read_text(encoding='utf-8')) if patches_path.exists() else []
    for spec in patches:
        package = spec['package']
        if not re.fullmatch(r'[A-Za-z0-9_-]+', package):
            raise RuntimeError(f'Invalid legacy source patch package: {package}')
        relative = spec.get('path', f'examples/{package}.ts')
        if Path(relative).is_absolute() or '..' in Path(relative).parts:
            raise RuntimeError(f'Invalid legacy source patch path: {relative}')
        target = legacy/relative
        content = (staged[target] if target in staged else target.read_bytes()).decode('utf-8-sig').replace('\r\n', '\n')
        if spec['new'] and spec['new'] in content:
            pass
        elif spec['old'] in content:
            content = content.replace(spec['old'], spec['new'])
        elif spec['new'] not in content:
            raise RuntimeError(f'Operit1 source patch needs review: {relative}')
        staged[target] = content.encode()
        digest.update(json.dumps(spec, sort_keys=True).encode())
    for target, data in staged.items():
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    return digest.hexdigest()

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
    adapted = adapter_methods()
    missing = sorted(set(required)-available-adapted)
    host = host_dependencies(body)
    check_native_contracts(host['legacy_host_class_dependencies'], package_id)
    if missing:
        raise RuntimeError(f'Legacy package {package_id} has unported Tools methods: {missing}')
    wrapped=('/* METADATA\n'+metadata.strip()+'\n*/\n'
             '// Adapted for the enhanced Operit2 channel; see bundled legacy report and LICENSE.\n'
             + wrap_host_script(body))
    return wrapped, {'file':filename,'original_id':original_id,'package_id':package_id,
        'enabled_by_default':False,'required_methods':required,'missing_methods':missing,
        'adapted_methods':sorted(set(required)&adapted),
        **host,
        'status':'host_adapters_bundled',
        'compiled_source_sha256':hashlib.sha256(source.encode()).hexdigest(),
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
                text = namespace_input_menu(text, original_id)
                if METADATA.search(text):
                    text,_ = convert(text,name,available)
                else:
                    text = wrap_host_script(text)
                data = text.encode()
                check_javascript(data, f'{folder.name}/{name}')
            entry=zipfile.ZipInfo(name,date_time=(2020,1,1,0,0,0))
            entry.compress_type=zipfile.ZIP_DEFLATED
            archive.writestr(entry,data)
    adapted=adapter_methods()
    missing=sorted(required-available-adapted)
    check_native_contracts(legacy_host_classes, package_id)
    if missing:
        raise RuntimeError(f'Legacy ToolPkg {package_id} has unported Tools methods: {missing}')
    return output.getvalue(), {'file':folder.name+'.toolpkg','original_id':original_id,
        'package_id':package_id,'enabled_by_default':False,'required_methods':sorted(required),
        'missing_methods':missing,'native_global_dependencies':sorted(native_globals),
        'adapted_methods':sorted(required&adapted),
        'legacy_host_class_dependencies':sorted(legacy_host_classes),
        'status':'host_adapters_bundled',
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
    source_identity = _prepare_legacy_sources(legacy, sha)
    _compile_legacy(legacy,[p.source for p in plans if p.mode=='pack'],source_identity)
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
    for family, filename in [('legacy-tools', 'contracts.json'),
                             ('legacy-tools', 'package-coverage.json'),
                             ('legacy-tools', 'contract-review.json'),
                             ('legacy-native', 'native-contracts.json')]:
        (report_dir/filename).write_bytes((ENHANCEMENTS/family/filename).read_bytes())
    (report_dir/'UPSTREAM-LICENSE').write_bytes((legacy/'LICENSE').read_bytes())
    (report_dir/'README.txt').write_text('Legacy packages are compiled from the official whitelist with the enhanced source ports and local Tools adapters, namespaced and disabled by default. The manifest records bundled host adapters; it does not claim every external service or Android device has been exercised. Android permissions, service credentials and remote companion software remain required where the original package requires them. Original sources and license are in Operit-legacy-source.zip; enhanced source changes are recorded alongside the build. Enable only one implementation of equivalent input hooks.\n',encoding='utf-8')
    print(f'Legacy whitelist: {len(planned)} packages bundled, including ToolPkg resources.',flush=True)
    return result
