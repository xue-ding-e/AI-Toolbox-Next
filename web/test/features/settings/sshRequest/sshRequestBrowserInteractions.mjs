const delay = () => new Promise(resolve => setTimeout(resolve, 20));

export async function waitForSSHFixtureState(predicate, label = 'UI state') {
  for (let attempt = 0; attempt < 200; attempt++) {
    if (await predicate()) return;
    await delay();
  }
  throw Error(`${label} deadline`);
}

// Runs in the synthetic fixture only. The same readiness predicate supplies the
// eventual click point; a nonzero option height alone is not enough during motion.
async function inspectClickTarget(selector, exactText) {
  await new Promise(resolve => requestAnimationFrame(resolve));
  const visible = element => {
    const rect = element.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return false;
    for (let owner = element; owner; owner = owner.parentElement) {
      const style = getComputedStyle(owner);
      if (style.visibility !== 'visible' || Number(style.opacity) === 0) return false;
    }
    return true;
  };
  const targets = [...document.querySelectorAll(selector)].filter(element =>
    (exactText === null || element.textContent === exactText) && visible(element));
  if (targets.length !== 1) return {reason: `visible target count ${targets.length}`};
  const element = targets[0];
  if (element.closest(':disabled, [aria-disabled="true"]') || getComputedStyle(element).pointerEvents === 'none') {
    return {reason: 'target disabled or pointer events blocked'};
  }
  // Select options are in a body Portal, not the Modal subtree. Wait only for
  // this target and its ancestors, never unrelated descendants or infinite spins.
  for (let owner = element; owner; owner = owner.parentElement) {
    if (owner.getAnimations().some(animation =>
      (animation.playState === 'running' || animation.pending) &&
      Number.isFinite(animation.effect?.getComputedTiming().endTime))) {
      return {reason: 'target or ancestor finite motion'};
    }
  }
  const rect = element.getBoundingClientRect();
  const x = rect.x + rect.width / 2;
  const y = rect.y + rect.height / 2;
  if (x < 0 || y < 0 || x >= innerWidth || y >= innerHeight || !element.contains(document.elementFromPoint(x, y))) {
    return {reason: 'target occluded or outside viewport'};
  }
  return {point: {x, y}, bounds: [rect.x, rect.y, rect.width, rect.height]};
}

export function createSSHFixtureInteractions({send, evaluate, snapshot, drain, onPhase}) {
  const categories = new Map([
    ['.ant-modal .ant-select', 'connection-select'],
    ['.ant-select-item-option-content > div > span', 'connection-option'],
    ['.ant-modal-close', 'modal-close'],
    ['.ant-modal button[role="switch"]', 'enable-switch'],
    ['#reopen', 'reopen-button'],
    ['#unmount', 'unmount-button'],
  ]);
  const click = async (selector, exactText = null) => {
    const category = categories.get(selector);
    if (!category) throw Error('Unknown SSH fixture selector category');
    onPhase('wait-click-target', category);
    let previous;
    let stableFrames = 0;
    let point;
    let reason = 'target not sampled';
    for (let attempt = 0; attempt < 200; attempt++) {
      const sample = await evaluate(`(${inspectClickTarget.toString()})(${JSON.stringify(selector)},${JSON.stringify(exactText)})`);
      if (sample.point) {
        const current = JSON.stringify([sample.bounds, sample.point]);
        stableFrames = current === previous ? stableFrames + 1 : 1;
        previous = current;
        reason = 'click point not stable across three animation frames';
        if (stableFrames === 3) {point = sample.point; break;}
      } else {
        stableFrames = 0;
        previous = undefined;
        reason = sample.reason;
      }
      await delay();
    }
    if (!point) throw Error(`${category}: click readiness deadline (${reason})`);
    onPhase('dispatch-click', category);
    // Exactly one real click. Do not retry a missed click or invoke React handlers.
    await send('Input.dispatchMouseEvent', {type: 'mousePressed', button: 'left', clickCount: 1, ...point});
    await send('Input.dispatchMouseEvent', {type: 'mouseReleased', button: 'left', clickCount: 1, ...point});
  };
  const chooseB = async () => {
    await click('.ant-modal .ant-select');
    // Product Option renders a name span followed by edit/delete buttons. Click
    // only the exact name, not the option center or an unguaranteed title attr.
    await click('.ant-select-item-option-content > div > span', 'Connection B');
    onPhase('wait-B-request', 'connection-option');
    await drain();
    await waitForSSHFixtureState(async () => (await snapshot()).calls.some(call => call.id === 'B' && !call.settled),
      'Connection B request after one click');
  };
  return {click, chooseB};
}
