"""Isolated PS5.1 probe of existing Rust command templates. No SSH/network/credentials.
Usage: python probe.py <repo-root> [--prepare-only]
Artifacts are kept in a new OS temp directory; no existing files are deleted.
"""
import base64
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

repo = pathlib.Path(sys.argv[1]).resolve()
source_path = repo / 'tauri/src/coding/ssh/sync.rs'
source = source_path.read_text(encoding='utf-8')
# Extract real Rust strings, not reconstructed equivalents. Fail if shape changes.
def rust_format_string(fragment, variable):
    pattern = r'let ' + re.escape(variable) + r' = format!\(\s*("(?:\\.|[^"\\])*")'
    matches = re.findall(pattern, fragment)
    if len(matches) != 1:
        raise RuntimeError(f'Expected exactly one {variable} template, got {len(matches)}')
    return json.loads(matches[0])

single = source.split('pub async fn sync_single_file_with_progress(', 1)[1].split('pub async fn sync_directory_with_progress(', 1)[0]
directory = source.split('pub async fn sync_directory_with_progress(', 1)[1].split('pub async fn ', 1)[0]
mkdir_template = rust_format_string(single, 'mkdir_cmd')
swap_template = rust_format_string(directory, 'swap_cmd')
# write_remote_file uses a separate command variable; retain the actual string.
# Select using full format! rather than a loose partial match.
formats = re.findall(r'format!\(\s*("(?:\\.|[^"\\])*")', source)
write_templates = [json.loads(s) for s in formats if 'cat >' in s and 'dirname' in s]
if len(write_templates) != 1:
    raise RuntimeError('Expected one write template')
root = pathlib.Path(tempfile.mkdtemp(prefix='ai-ssh433-'))
cases = [
    ('mkdir', mkdir_template, 'no PS parser error expected; runtime compatibility is measured'),
    ('directory_swap', swap_template, 'PS5.1 parser error and nonzero execution expected; no mutation'),
    ('write_file', write_templates[0], 'PS5.1 parser error and nonzero execution expected; no mutation'),
]
def substitute(template, args):
    for value in args:
        template = template.replace('{}', value.replace('\\', '/'), 1)
    if '{}' in template:
        raise RuntimeError('Unfilled placeholder')
    return template
manifest = {
    'source': str(source_path),
    'source_sha256': hashlib.sha256(source_path.read_bytes()).hexdigest(),
    'source_git_blob_sha1': hashlib.sha1(b'blob ' + str(len(source_path.read_bytes())).encode('ascii') + b'\0' + source_path.read_bytes()).hexdigest(),
    'root': str(root),
    'cases': [],
    'scope': 'Local shell only, not Windows Server 2022 SSH or application acceptance. No encoding autodetection claim.',
}
for name, template, expectation in cases:
    case_root = root / name / 'space 中文'
    target = case_root / 'target'
    staged = case_root / 'staged'
    staged.mkdir(parents=True)
    (staged / 'staged-marker.txt').write_text('staged fixture', encoding='utf-8')
    if name == 'directory_swap':
        target.mkdir()
        (target / 'target-marker.txt').write_text('original fixture', encoding='utf-8')
    file = target / 'config.json'
    for path in (target, staged, file):
        path.resolve().relative_to(root.resolve())
    args = [str(file)] if name == 'mkdir' else ([str(target), str(staged), str(target)] if name == 'directory_swap' else [str(file)] * 2)
    manifest['cases'].append({'name': name, 'template': template, 'command': substitute(template, args),
        'expectation': expectation, 'paths': {'target': str(target), 'staged': str(staged), 'file': str(file)}})
(root / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
print(root)
if '--prepare-only' in sys.argv:
    sys.exit(0)
shell = shutil.which('powershell.exe')
if not shell:
    raise RuntimeError('Windows PowerShell 5.1 required; do not substitute pwsh')
def invoke(script):
    encoded = base64.b64encode(script.encode('utf-16le')).decode('ascii')
    return subprocess.run([shell, '-NoProfile', '-NonInteractive', '-EncodedCommand', encoded], input=b'', capture_output=True, timeout=15)
version = invoke('$PSVersionTable.PSVersion.ToString()')
if not version.stdout.strip().startswith(b'5.1.'):
    raise RuntimeError(f'Expected 5.1, got {version.stdout!r}')
manifest['version_stdout_base64'] = base64.b64encode(version.stdout).decode()
def capture(case, phase, result):
    item = {'exit_code': result.returncode}
    for stream in ['stdout', 'stderr']:
        data = getattr(result, stream)
        (root / (case['name'] + '.' + phase + '.' + stream + '.bin')).write_bytes(data)
        item[stream + '_base64'] = base64.b64encode(data).decode()
        try:
            item[stream + '_utf8'] = data.decode('utf-8')
        except UnicodeDecodeError:
            item[stream + '_utf8'] = None
    case[phase] = item

def snapshot(paths):
    result = {}
    for label, value in paths.items():
        path = pathlib.Path(value)
        entries = sorted(path.rglob('*')) if path.is_dir() else [path]
        result[label] = {'exists': path.exists(), 'is_directory': path.is_dir(),
            'files': {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
                      for p in entries if p.is_file()}}
    return result

for case in manifest['cases']:
    case['before'] = snapshot(case['paths'])
    # Native positive control is isolated from the paths under test.
    control_dir = pathlib.Path(case['paths']['target']).parent / 'positive control 中文'
    control_file = control_dir / 'write.txt'
    control_dir.resolve().relative_to(root.resolve())
    d = str(control_dir).replace("'", "''")
    f = str(control_file).replace("'", "''")
    control = invoke("$ErrorActionPreference='Stop'; [void][System.IO.Directory]::CreateDirectory('" + d + "'); [System.IO.File]::WriteAllText('" + f + "','fixture-ok'); if([System.IO.File]::ReadAllText('" + f + "') -ne 'fixture-ok'){exit 2}")
    capture(case, 'positive_control', control)
    case['positive_control_passed'] = control.returncode == 0 and control_file.read_bytes() == b'fixture-ok' if control_file.exists() else False
    # Parse without running, then execute the exact command in a separate process.
    literal = case['command'].replace("'", "''")
    parser = invoke("$tokens=$null; $errors=$null; [void][System.Management.Automation.Language.Parser]::ParseInput('" + literal + "',[ref]$tokens,[ref]$errors); $errors | ForEach-Object { $_.ErrorId + ': ' + $_.Message }; if($errors.Count){exit 1}")
    capture(case, 'parse', parser)
    if case['positive_control_passed']:
        actual = invoke(case['command'])
        capture(case, 'execute', actual)
        case['after'] = snapshot(case['paths'])
        case['observed_no_mutation'] = case['before'] == case['after']
        if case['name'] == 'mkdir':
            case['parser_expectation_met'] = parser.returncode == 0
            case['verdict'] = 'runtime measured, not an asserted incompatibility regression'
        else:
            case['expectation_met'] = parser.returncode != 0 and actual.returncode != 0 and case['observed_no_mutation']
            case['verdict'] = 'expected incompatibility observed' if case['expectation_met'] else 'unexpected result, investigate'
    else:
        case['verdict'] = 'fixture control failed; command execution skipped, inconclusive'
    (root / 'results.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
print('Measurement complete; consult per-case verdicts, not process success. No SSH operation performed.')
if any(not c['positive_control_passed'] or c.get('expectation_met') is False or c.get('parser_expectation_met') is False for c in manifest['cases']):
    sys.exit(1)
