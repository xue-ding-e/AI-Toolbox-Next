"""Draft: real native WebDriver smoke; no invoke mocking or storage writes."""
import base64, hashlib, json, os, pathlib, sqlite3, time, urllib.request, urllib.error, shutil
ROOT = pathlib.Path(os.environ['SMOKE_ROOT']).resolve()
BINARY = pathlib.Path(os.environ['SMOKE_BINARY']).resolve()
EVIDENCE = pathlib.Path(os.environ['SMOKE_EVIDENCE']).resolve()
BASE = 'http://127.0.0.1:4444'
ELEMENT = 'element-6066-11e4-a52e-4f735466cecf'
class NativeSafetyStop(Exception): pass

session = None
stages = []
locator_observations = {}

def record(stage, **details):
    stages.append({"stage": stage, "status": "passed", **details})
    (EVIDENCE / "stages.json").write_text(json.dumps(stages, indent=2))

def request(method, path, body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(BASE + path, data=data, method=method, headers={'Content-Type': 'application/json'})
    try:
        with urllib.request.urlopen(req, timeout=35) as response:
            result = json.load(response).get('value')
    except urllib.error.HTTPError as error:
        raise RuntimeError(f'WebDriver {method} {path}: HTTP {error.code}: {error.read().decode()}') from error
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
        except NativeSafetyStop: raise
        except Exception as error: last = str(error)
        time.sleep(.2)
    raise AssertionError(f'Timeout: {last}')

def xpath(value):
    matches=request('POST', f'/session/{session}/elements', {'using':'xpath','value':value})
    displayed=[request('GET', f'/session/{session}/element/{entry[ELEMENT]}/displayed') for entry in matches]
    locator_observations[value]={'count':len(matches),'displayed':displayed}
    (EVIDENCE / 'locator-observations.json').write_text(json.dumps(locator_observations,indent=2))
    assert len(matches)==1 and displayed==[True], locator_observations[value]
    return matches[0][ELEMENT]

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


def collect_popup_diagnostics(label):
    # Observation only. No style, DOM, storage or application mutations.
    script = """
      const selectors=['.ant-select-open','.ant-select-open input[role=combobox]',
        '.ant-select-dropdown','.rc-virtual-list-holder','.rc-virtual-list-holder-inner',
        '.ant-select-item-option','.ant-select-item-option-content'];
      const info = element => {
        const rect=element.getBoundingClientRect(), css=getComputedStyle(element);
        return {tag:element.tagName,classes:element.className,text:(element.textContent || "").slice(0,300),
          rect:{x:rect.x,y:rect.y,width:rect.width,height:rect.height},
          style:Object.fromEntries(['display','visibility','opacity','overflow','overflow-x','overflow-y',
            'position','z-index','transform','clip','clip-path','content-visibility','animation-name',
            'animation-play-state','pointer-events'].map(key=>[key,css.getPropertyValue(key)]))};
      };
      return {viewport:{width:innerWidth,height:innerHeight,dpr:devicePixelRatio},
        elements:selectors.flatMap(selector=>Array.from(document.querySelectorAll(selector)).map(element=>{
          const ancestors=[];for(let p=element.parentElement;p;p=p.parentElement)ancestors.push(info(p));
          return {selector,...info(element),ancestors};
        }))};
    """
    errors={}
    try:
        shutil.copyfile(ROOT/'xvfb-framebuffers'/'Xvfb_screen0', EVIDENCE/(label+'.xwd'))
    except Exception as error: errors['native-framebuffer']=str(error)
    try:
        value=request('POST', f'/session/{session}/execute/sync', {'script':script,'args':[]})
        (EVIDENCE / (label+'-geometry.json')).write_text(json.dumps(value,indent=2))
    except Exception as error: errors['geometry']=str(error)
    try:
        (EVIDENCE / (label+'-source.html')).write_text(request('GET',f'/session/{session}/source'))
    except Exception as error: errors['source']=str(error)
    try: screenshot(label+'.png')
    except Exception as error: errors['screenshot']=str(error)
    (EVIDENCE / (label+'-diagnostic-errors.json')).write_text(json.dumps(errors,indent=2))


def ssh_snapshot(label):
    import subprocess
    result = subprocess.run([os.environ['SMOKE_NODE'], str(pathlib.Path(__file__).with_name('ssh_snapshot.cjs')), str(ROOT/'data'/'ai-toolbox.db')], check=True, capture_output=True, text=True, timeout=10)
    value = json.loads(result.stdout)
    (EVIDENCE / ('ssh-'+label+'.json')).write_text(json.dumps(value, indent=2))
    # A nonempty connection/active state is a stop condition, never auto-cleaned.
    if value['count'] != 0 or value['active'] != '':
        raise NativeSafetyStop(f'Unexpected SSH state; no recovery attempted: {value}')
    return value

def ssh_cancel_smoke():
    modal = "//div[@role='dialog'][.//div[contains(concat(' ',normalize-space(@class),' '),' ant-modal-title ') and normalize-space()='SSH Config Sync Settings']]"
    form = "//div[@role='dialog'][.//div[contains(concat(' ',normalize-space(@class),' '),' ant-modal-title ') and normalize-space()='New Connection']]"
    enabled = modal + "//span[normalize-space()='Enable SSH Sync']/following-sibling::button[@role='switch']"
    new = modal + "//span[normalize-space()='Active Connection:']/ancestor::div[.//button[.//span[@aria-label='plus']]][1]//button[.//span[@aria-label='plus']]"
    name = form + "//input[@id='name']"
    cancel = form + "//button[span[normalize-space()='Cancel']]"
    click("//div[contains(concat(' ',normalize-space(@class),' '),' ssh-status-indicator ')]")
    wait(lambda: xpath(enabled))
    before = ssh_snapshot('before-enable')
    assert before['enabled'] == 0 and attr(enabled, 'aria-checked') == 'false'
    click(enabled)
    wait(lambda: attr(enabled, 'aria-checked') == 'true')
    wait(lambda: ssh_snapshot('enabled')['enabled'] == 1)
    click(new)
    wait(lambda: xpath(name))
    assert attr(name, 'value') == ''
    field = xpath(name)
    request('POST', f'/session/{session}/element/{field}/value', {'text':'cancel-only-synthetic-name'})
    assert attr(name, 'value') == 'cancel-only-synthetic-name'
    screenshot('ssh-draft-name.png')
    # Observe focus without mutating it; send a real WebDriver Escape key pair.
    focused = request('GET', f'/session/{session}/element/active')
    assert focused[ELEMENT] == field, 'Synthetic name input must retain focus'
    (EVIDENCE / 'ssh-escape-focus.json').write_text(json.dumps({'name_input_focused': True}))
    request('POST', f'/session/{session}/actions', {'actions': [{
        'type': 'key', 'id': 'ssh-cancel-keyboard', 'actions': [
            {'type': 'keyDown', 'value': '\ue00c'}, {'type': 'keyUp', 'value': '\ue00c'}]}]})
    wait(lambda: not request('POST', f'/session/{session}/elements', {'using':'xpath','value':form}))
    xpath(modal)  # Escape closes only the child; the parent must remain visible.
    ssh_snapshot('after-escape')
    click(new)
    wait(lambda: xpath(name))
    expected = {'name':'', 'host':'', 'port':'22', 'username':'', 'privateKeyPath':'', 'passphrase':''}
    observed = {key:attr(form + "//*[@id='"+key+"']", 'value') for key in expected}
    assert observed == expected, observed
    selected = form + "//label[contains(concat(' ',normalize-space(@class),' '),' ant-radio-wrapper-checked ')][.//input[@value='key']]"
    xpath(selected)
    ssh_snapshot('reopened')
    screenshot('ssh-reopened-defaults.png')
    click(cancel)
    wait(lambda: not request('POST', f'/session/{session}/elements', {'using':'xpath','value':form}))
    click(enabled)
    wait(lambda: attr(enabled, 'aria-checked') == 'false')
    wait(lambda: ssh_snapshot('restored-disabled')['enabled'] == 0)
    click(modal + "//button[@aria-label='Close']")
    # Wait for the actual modal exit; displayed underlying controls can still be masked.
    wait(lambda: request('POST', f'/session/{session}/execute/sync', {
        'script': "return !document.querySelector('.ant-modal-mask') && Array.from(document.querySelectorAll('.ant-modal-wrap')).every(e => getComputedStyle(e).display === 'none')", 'args': []}))
    record('ssh-unsaved-name-escape-reopen', connection_count=0, active_connection='', restored_enabled=False)

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
    wait(lambda: xpath(THEME))
    return next(iter(current))

THEME = "//*[normalize-space()='Current Theme:' or normalize-space()='当前主题:']/parent::div//div[contains(concat(' ',normalize-space(@class),' '),' ant-select ')]"
CLOSE = "//span[normalize-space()='Minimize to Tray on Close' or normalize-space()='关闭时最小化到托盘']/following-sibling::button[@role='switch']"

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
    ssh_cancel_smoke()
    old = settings_fingerprint()
    click(THEME)
    collect_popup_diagnostics('popup-open')
    click("//div[contains(concat(' ',normalize-space(@class),' '),' ant-select-item-option ')][div[contains(concat(' ',normalize-space(@class),' '),' ant-select-item-option-content ') and (normalize-space()='" + target_labels[0] + "' or normalize-space()='" + target_labels[1] + "')]]")
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
        try:
            (EVIDENCE / 'failure.html').write_text(request('GET', f'/session/{session}/source'))
        except Exception as source_error:
            (EVIDENCE / 'failure-source-error.txt').write_text(str(source_error))
        try: screenshot('failure.png')
        except Exception as screenshot_error:
            (EVIDENCE / 'failure-screenshot-error.txt').write_text(str(screenshot_error))
        collect_popup_diagnostics('popup-failure')
    raise
