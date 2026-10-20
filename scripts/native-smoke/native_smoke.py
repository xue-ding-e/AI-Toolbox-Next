"""Draft: real native WebDriver smoke; no invoke mocking or storage writes."""
import base64, hashlib, json, os, pathlib, sqlite3, time, urllib.request
ROOT = pathlib.Path(os.environ['SMOKE_ROOT']).resolve()
BINARY = pathlib.Path(os.environ['SMOKE_BINARY']).resolve()
EVIDENCE = pathlib.Path(os.environ['SMOKE_EVIDENCE']).resolve()
BASE = 'http://127.0.0.1:4444'
ELEMENT = 'element-6066-11e4-a52e-4f735466cecf'
session = None
stages = []

def record(stage, **details):
    stages.append({"stage": stage, "status": "passed", **details})
    (EVIDENCE / "stages.json").write_text(json.dumps(stages, indent=2))

def request(method, path, body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(BASE + path, data=data, method=method, headers={'Content-Type': 'application/json'})
    with urllib.request.urlopen(req, timeout=35) as response:
        result = json.load(response).get('value')
    if isinstance(result, dict) and result.get('error'):
        raise RuntimeError(result)
    return result

def wait(predicate, seconds=40):
    deadline = time.monotonic() + seconds
    last = None
    while time.monotonic() < deadline:
        try:
            value = predicate()
            if value: return value
        except Exception as error: last = str(error)
        time.sleep(.2)
    raise AssertionError(f'Timeout: {last}')

def xpath(value):
    return request('POST', f'/session/{session}/element', {'using': 'xpath', 'value': value})[ELEMENT]

def click(value):
    element = wait(lambda: xpath(value))
    request('POST', f'/session/{session}/element/{element}/click', {})

def attr(value, name):
    element = xpath(value)
    return request('GET', f'/session/{session}/element/{element}/attribute/{name}')

def text(value):
    element = xpath(value)
    return request('GET', f'/session/{session}/element/{element}/text')

def pids():
    result = set()
    for entry in pathlib.Path('/proc').iterdir():
        if entry.name.isdigit():
            try:
                if (entry / 'exe').resolve() == BINARY: result.add(int(entry.name))
            except OSError: pass
    return result

def settings_fingerprint():
    # JSONB stays opaque: system SQLite need not decode app-bundled SQLite JSONB.
    # Only one disposable settings row is read; no database snapshot is uploaded.
    path = ROOT / 'data' / 'ai-toolbox.db'
    if not path.exists(): return None
    with sqlite3.connect(f'file:{path}?mode=ro', uri=True) as conn:
        columns={row[1]:row[2].upper() for row in conn.execute('PRAGMA table_info(settings)')}
        assert columns.get('data') == 'BLOB' and 'id' in columns, columns
        row = conn.execute("SELECT data FROM settings WHERE id = 'app'").fetchone()
    return hashlib.sha256(row[0]).hexdigest() if row else None

def screenshot(name):
    encoded = request('GET', f'/session/{session}/screenshot')
    (EVIDENCE / name).write_bytes(base64.b64decode(encoded))

def launch():
    global session
    before = pids()
    assert not before, f'Previous app still running: {before}'
    result = request('POST', '/session', {'capabilities': {'alwaysMatch': {'tauri:options': {'application': str(BINARY)}}}})
    session = result['sessionId']
    current = wait(pids)
    assert len(current) == 1, current
    # Read-only DOM/environment inspection. No injected invocation or mutations.
    wait(lambda: request('POST', f'/session/{session}/execute/sync', {'script': 'return !!window.__TAURI_INTERNALS__ && document.body.innerText.length > 20', 'args': []}))
    click("//header//span[normalize-space()='Settings' or normalize-space()='设置']/parent::div")
    wait(lambda: xpath("//*[normalize-space()='Current Theme:' or normalize-space()='当前主题:']/parent::div//div[contains(@class,'ant-select')]") )
    return next(iter(current))

THEME = "//*[normalize-space()='Current Theme:' or normalize-space()='当前主题:']/parent::div//div[contains(@class,'ant-select-selector')]"
CLOSE = "//*[normalize-space()='Minimize to Tray on Close' or normalize-space()='关闭时最小化到托盘']/parent::div//button[@role='switch']"

def close_normally():
    global session
    # A failed/unsupported native window close is a test limitation, not success.
    request('DELETE', f'/session/{session}/window')
    wait(lambda: not pids(), 20)
    session = None

EVIDENCE.mkdir(parents=True, exist_ok=True)
try:
    first = launch()
    screenshot('first-launch.png')
    initial_theme = text(THEME)
    assert initial_theme in ('Dark','深色','Light','浅色','System','Follow System','跟随系统'), initial_theme
    target_labels = ('Light','浅色') if initial_theme in ('Dark','深色') else ('Dark','深色')
    record('native-empty-profile-rendered', pid=first, initial_theme=initial_theme)
    old = settings_fingerprint()
    click(THEME)
    click("//div[contains(@class,'ant-select-item-option-content') and (normalize-space()='" + target_labels[0] + "' or normalize-space()='" + target_labels[1] + "')]")
    wait(lambda: text(THEME) in target_labels)
    wait(lambda: settings_fingerprint() not in (None, old))
    record('theme-ui-changed-and-save-barrier', target_theme=target_labels[0])
    before_close = settings_fingerprint()
    assert attr(CLOSE, 'aria-checked') == 'true', 'Empty profile close-to-tray default differs'
    click(CLOSE)
    wait(lambda: attr(CLOSE, 'aria-checked') == 'false')
    wait(lambda: settings_fingerprint() not in (None, before_close))
    record('close-to-tray-disabled-and-save-barrier')
    screenshot('theme-saved.png')
    close_normally()
    record('first-normal-native-exit')
    second = launch()
    assert first != second, 'Expected a fresh native process'
    assert text(THEME) in target_labels, 'Theme did not survive native restart'
    assert attr(CLOSE, 'aria-checked') == 'false', 'Close setting did not persist'
    record('fresh-native-restart-persistence', pid=second, theme=text(THEME))
    screenshot('restarted.png')
    close_normally()
    record('second-normal-native-exit')
    (EVIDENCE / 'result.json').write_text(json.dumps({'passed': ['native empty-profile launch', 'theme UI save', 'normal window close', 'fresh PID restart', 'theme persistence', 'normal second exit'], 'first_pid': first, 'second_pid': second, 'binary_sha256': hashlib.sha256(BINARY.read_bytes()).hexdigest()}, indent=2))
except Exception as error:
    (EVIDENCE / 'failure.txt').write_text(str(error))
    (EVIDENCE / 'failed-stage.json').write_text(json.dumps({'completed': stages, 'error':str(error)},indent=2))
    if session:
        try: screenshot('failure.png')
        except Exception: pass
    raise
