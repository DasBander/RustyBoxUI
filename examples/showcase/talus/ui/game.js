import { rustybox, createPreviewClient } from './sdk/rustybox.js';
import './sdk/titlebar.js';
import { CanyonRenderer } from './renderer.js';
import { FlightAudio } from './audio.js';
import { courseMenu } from './course-packs.js';
import {
  newFlight,
  stepFlight,
  canyonCenter,
  GATES,
  GATE_RADIUS,
  COURSE_LENGTH,
  COURSE_NAME,
  useCourse,
  clock,
  clamp,
} from './course.js';

const $ = (id) => document.getElementById(id);
const canvas = $('world'),
  panel = $('panel'),
  audio = new FlightAudio();
const client = rustybox.isNative ? rustybox : createPreviewClient();
const profileKey = 'talus.flight.v1';
const preferences = { volume: 45, quality: 'balanced', invert: false };
const records = { copper: null, switchback: null };
let course = 'copper';
let saveQueue = Promise.resolve();
let phase = 'menu',
  currentPanel = '',
  panelParent = '',
  focusBeforePanel;
let renderer,
  flight,
  countdown = 3,
  resumePhase = 'flying',
  lastDigit = 0;
let raf = 0,
  previousFrame = 0,
  accumulator = 0,
  feedbackUntil = 0;
const held = new Set();
let stick = null,
  boostPointer = null;

function announce(message) {
  $('notice').textContent = message;
  $('notice').hidden = !message;
}

function updateBest() {
  $('best').textContent = records[course] === null ? 'Unflown' : clock(records[course]);
}
function updateSettings() {
  $('volume').value = preferences.volume;
  $('volume-value').textContent = preferences.volume + '%';
  $('quality').value = preferences.quality;
  $('invert').checked = preferences.invert;
  audio.setVolume(preferences.volume / 100);
  if (renderer) renderer.quality = preferences.quality;
}

async function loadProfile() {
  try {
    const data = await client.invoke('storage.get', { key: profileKey }, { timeout: 2500 });
    if (data) {
      if (Number.isFinite(data.best) && data.best > 0) records.copper = data.best;
      if (Number.isFinite(data.switchbackBest) && data.switchbackBest > 0)
        records.switchback = data.switchbackBest;
      preferences.course = data.course;
      if (Number.isFinite(data.volume)) preferences.volume = clamp(data.volume, 0, 100);
      if (['balanced', 'high'].includes(data.quality)) preferences.quality = data.quality;
      preferences.invert = data.invert === true;
    }
  } catch {
    announce('Your flight profile could not be opened. You can still fly.');
  }
  updateBest();
  updateSettings();
}

function saveProfile() {
  const value = {
    ...preferences,
    course,
    best: records.copper,
    switchbackBest: records.switchback,
  };
  saveQueue = saveQueue
    .then(() => client.invoke('storage.set', { key: profileKey, value }))
    .catch(() => announce('Could not save this flight profile.'));
}

function routeMap() {
  const map = $('route-map'),
    ctx = map.getContext('2d');
  const point = (z) => [5 + (z / COURSE_LENGTH) * 310, 40 - canyonCenter(z) * 0.69];
  ctx.clearRect(0, 0, map.width, map.height);
  ctx.strokeStyle = '#ecc4a8';
  ctx.lineWidth = 2.2;
  ctx.beginPath();
  for (let z = 0; z <= COURSE_LENGTH; z += 6) {
    const [x, y] = point(z);
    if (z === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }
  ctx.stroke();
  ctx.fillStyle = '#ff8d57';
  for (const gate of GATES) {
    const [x, y] = point(gate.z);
    ctx.fillRect(x - 2.4, y - 2.4, 4.8, 4.8);
  }
}

const panelTitles = {
  paused: 'Paused',
  settings: 'Settings',
  controls: 'How to fly',
  results: 'Flight complete',
};
function showPanel(name, parent = '') {
  if (!panel.open) focusBeforePanel = document.activeElement;
  currentPanel = name;
  panelParent = parent;
  for (const section of panel.querySelectorAll('[data-panel]'))
    section.hidden = section.dataset.panel !== name;
  $('panel-title').textContent = panelTitles[name];
  $('course-settings').hidden = phase !== 'menu';
  $('panel-context').textContent =
    name === 'controls' || name === 'settings' ? 'Flight deck' : COURSE_NAME;
  $('back').hidden = name === 'results';
  if (!panel.open) panel.showModal();
  const first = panel.querySelector('section:not([hidden]) button, section:not([hidden]) input');
  first?.focus({ preventScroll: true });
}
function closePanel() {
  if (panel.open) panel.close();
  currentPanel = '';
  panelParent = '';
}
function goBack() {
  if (currentPanel === 'paused') return resumeFlight();
  if (currentPanel === 'results') return mainMenu();
  if (panelParent === 'paused') return showPanel('paused');
  closePanel();
  if (focusBeforePanel?.isConnected) focusBeforePanel.focus({ preventScroll: true });
}
panel.addEventListener('cancel', (e) => {
  e.preventDefault();
  goBack();
});
$('back').onclick = goBack;
for (const button of document.querySelectorAll('.done')) button.onclick = goBack;
$('settings').onclick = () => showPanel('settings');
$('controls').onclick = () => showPanel('controls');
$('pause-settings').onclick = () => showPanel('settings', 'paused');
$('volume').oninput = () => {
  preferences.volume = Number($('volume').value);
  updateSettings();
};
$('volume').onchange = () => {
  audio.unlock();
  audio.note(510);
  saveProfile();
};
$('quality').onchange = () => {
  preferences.quality = $('quality').value;
  updateSettings();
  saveProfile();
  drawStill();
};
$('invert').onchange = () => {
  preferences.invert = $('invert').checked;
  saveProfile();
};

let changingDisplay = false;
let display = { enabled: false, supported: false };
function displayState(state) {
  display = state;
  document.documentElement.dataset.windowMode = state.enabled ? 'fullscreen' : 'windowed';
  for (const btn of document.querySelectorAll('[data-fullscreen]')) {
    btn.querySelector('.screen-label').textContent = state.enabled
      ? 'Exit fullscreen'
      : 'Fullscreen';
    btn.setAttribute('aria-pressed', String(state.enabled));
    btn.disabled = !state.supported;
  }
}
async function refreshDisplay() {
  try {
    displayState(await client.invoke('window.fullscreen'));
  } catch {
    displayState({ enabled: false, supported: false });
  }
}
async function toggleFullscreen() {
  if (changingDisplay || !display.supported) return;
  changingDisplay = true;
  try {
    displayState(await client.invoke('window.fullscreen', { enabled: !display.enabled }));
  } catch (error) {
    announce('Could not change display mode. ' + error.message);
  } finally {
    changingDisplay = false;
  }
}
document.addEventListener('fullscreenchange', refreshDisplay);
for (const btn of document.querySelectorAll('[data-fullscreen]')) btn.onclick = toggleFullscreen;
$('native-controls').hidden = !rustybox.isNative;
$('quit').hidden = !rustybox.isNative;
for (const btn of document.querySelectorAll('[data-window]'))
  btn.onclick = () =>
    rustybox.invoke('window.' + btn.dataset.window).catch((e) => announce(e.message));
$('quit').onclick = () => rustybox.invoke('window.close').catch((e) => announce(e.message));

function clearInput() {
  held.clear();
  if (stick && canvas.hasPointerCapture(stick.id)) canvas.releasePointerCapture(stick.id);
  stick = null;
  boostPointer = null;
}
function input() {
  const key = (...codes) => (codes.some((code) => held.has(code)) ? 1 : 0);
  return {
    x: clamp(key('KeyD', 'ArrowRight') - key('KeyA', 'ArrowLeft') + (stick?.x ?? 0), -1, 1),
    y:
      clamp(key('KeyW', 'ArrowUp') - key('KeyS', 'ArrowDown') + (stick?.y ?? 0), -1, 1) *
      (preferences.invert ? -1 : 1),
    boost: held.has('Space') || boostPointer !== null,
  };
}
const flightKeys = [
  'KeyA',
  'KeyD',
  'KeyW',
  'KeyS',
  'ArrowLeft',
  'ArrowRight',
  'ArrowUp',
  'ArrowDown',
  'Space',
];
window.addEventListener('keydown', (e) => {
  if (e.code === 'F11') {
    e.preventDefault();
    if (!e.repeat) toggleFullscreen();
    return;
  }
  if (panel.open) {
    if (e.code === 'KeyP' && currentPanel === 'paused' && !e.repeat) {
      e.preventDefault();
      resumeFlight();
    }
    return;
  }
  if (phase === 'menu' && e.code === 'Escape' && display.enabled) {
    e.preventDefault();
    toggleFullscreen();
    return;
  }
  if (phase === 'menu' && ['ArrowDown', 'ArrowUp'].includes(e.code)) {
    e.preventDefault();
    const buttons = [...document.querySelectorAll('.menu-actions button')].filter(
      (b) => !b.hidden && !b.disabled
    );
    const index = buttons.indexOf(document.activeElement);
    buttons[(index + (e.code === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length]?.focus();
    return;
  }
  if (phase !== 'flying' && phase !== 'countdown') return;
  if (['KeyP', 'Escape'].includes(e.code)) {
    e.preventDefault();
    if (!e.repeat) pauseFlight();
    return;
  }
  if (flightKeys.includes(e.code)) {
    e.preventDefault();
    held.add(e.code);
  }
});
window.addEventListener('keyup', (e) => held.delete(e.code));
canvas.addEventListener('pointerdown', (e) => {
  if (e.button !== 0 || stick || !['flying', 'countdown'].includes(phase)) return;
  canvas.setPointerCapture(e.pointerId);
  stick = { id: e.pointerId, originX: e.clientX, originY: e.clientY, x: 0, y: 0 };
});
canvas.addEventListener('pointermove', (e) => {
  if (stick?.id !== e.pointerId) return;
  stick.x = clamp((e.clientX - stick.originX) / 65, -1, 1);
  stick.y = clamp((stick.originY - e.clientY) / 65, -1, 1);
});
canvas.addEventListener('lostpointercapture', (e) => {
  if (stick?.id === e.pointerId) stick = null;
});
$('boost').addEventListener('pointerdown', (e) => {
  if (e.button === 0) {
    $('boost').setPointerCapture(e.pointerId);
    boostPointer = e.pointerId;
  }
});
$('boost').addEventListener('lostpointercapture', (e) => {
  if (boostPointer === e.pointerId) boostPointer = null;
});
window.addEventListener('blur', () => {
  clearInput();
  pauseFlight();
});
document.addEventListener('visibilitychange', () => {
  if (document.hidden) {
    clearInput();
    pauseFlight();
  }
});

function ensureRenderer() {
  if (!renderer) {
    renderer = new CanyonRenderer(canvas);
    renderer.quality = preferences.quality;
  }
}
canvas.addEventListener('webglcontextlost', (e) => {
  e.preventDefault();
  renderer = null;
  pauseFlight();
  announce('Graphics were interrupted. Your flight is paused.');
});
canvas.addEventListener('webglcontextrestored', () => {
  try {
    ensureRenderer();
    announce('');
    drawStill();
  } catch (e) {
    announce(e.message);
  }
});
function drawStill() {
  if (renderer && flight && !$('field').hidden) {
    renderer.draw(flight);
    updateHUD();
  }
}
new ResizeObserver(() => {
  if (!raf) drawStill();
}).observe(canvas);

function updateHUD() {
  $('time').textContent = clock(flight.elapsed + flight.penalty);
  $('speed').textContent = Math.round(flight.speed * 3.6);
  $('gates').textContent =
    flight.next < GATES.length
      ? String(flight.next + 1).padStart(2, '0') + ' / ' + String(GATES.length).padStart(2, '0')
      : 'Finish';
  $('distance').value = flight.z;
  $('charge').value = flight.charge;
  const gate = GATES[flight.next],
    label = $('gate-label');
  const point = gate && renderer?.project(gate.x, gate.y + GATE_RADIUS + 1, gate.z);
  label.hidden =
    !point || point[0] < 0.045 || point[0] > 0.955 || point[1] < 0.09 || point[1] > 0.88;
  if (!label.hidden) {
    label.style.left = point[0] * 100 + '%';
    label.style.top = point[1] * 100 + '%';
    label.textContent =
      'G' +
      String(flight.next + 1).padStart(2, '0') +
      ' / ' +
      Math.max(0, Math.ceil(gate.z - flight.z)) +
      ' m';
  }
  if (flight.elapsed > feedbackUntil && $('feedback').textContent) $('feedback').textContent = '';
}
function showEvent(event) {
  const message = {
    gate: 'Gate cleared',
    miss: 'Missed gate  +4 s',
    scrape: 'Terrain contact  +2 s',
  }[event];
  if (!message) return;
  $('feedback').textContent = message;
  feedbackUntil = flight.elapsed + 1.25;
  if (event === 'gate') {
    audio.note(660);
    audio.note(990, 0.15, 0.07);
  } else audio.note(150, 0.17);
}
function stopFrames() {
  if (raf) cancelAnimationFrame(raf);
  raf = 0;
  previousFrame = 0;
  accumulator = 0;
  audio.air(false);
}
function nextFrame() {
  if (!raf) raf = requestAnimationFrame(frame);
}
function frame(stamp) {
  raf = 0;
  const dt = previousFrame ? clamp((stamp - previousFrame) / 1000, 0, 0.18) : 0;
  previousFrame = stamp;
  if (phase === 'countdown') {
    countdown -= dt;
    const digit = Math.ceil(countdown);
    if (digit !== lastDigit && digit > 0) {
      lastDigit = digit;
      audio.note(digit === 1 ? 620 : 440, 0.09);
    }
    $('countdown').textContent = Math.max(1, digit);
    if (countdown <= 0) {
      phase = 'flying';
      $('countdown').hidden = true;
      audio.note(880, 0.15);
    }
  } else if (phase === 'flying') {
    accumulator += dt;
    const controls = input(),
      step = 1 / 90;
    while (accumulator >= step && !flight.finished) {
      stepFlight(flight, controls, step);
      accumulator -= step;
      if (flight.lastEvent) showEvent(flight.lastEvent);
    }
    audio.air(true, flight.boosting);
  } else return;
  drawStill();
  if (flight.finished) finishFlight();
  else nextFrame();
}
function startFlight() {
  closePanel();
  clearInput();
  stopFrames();
  try {
    ensureRenderer();
  } catch (e) {
    mainMenu();
    announce(e.message);
    return;
  }
  audio.unlock();
  announce('');
  flight = newFlight();
  phase = 'countdown';
  countdown = 3;
  lastDigit = 0;
  feedbackUntil = 0;
  $('menu').hidden = true;
  $('field').hidden = false;
  $('countdown').hidden = false;
  $('countdown').textContent = '3';
  $('feedback').textContent = '';
  $('location').textContent = COURSE_NAME;
  canvas.focus({ preventScroll: true });
  nextFrame();
}
function pauseFlight() {
  if (phase !== 'flying' && phase !== 'countdown') return;
  resumePhase = phase;
  phase = 'paused';
  clearInput();
  stopFrames();
  showPanel('paused');
}
function resumeFlight() {
  if (phase !== 'paused') return;
  try {
    ensureRenderer();
  } catch (e) {
    announce(e.message);
    return;
  }
  closePanel();
  clearInput();
  phase = resumePhase;
  canvas.focus({ preventScroll: true });
  nextFrame();
}
function finishFlight() {
  phase = 'finished';
  clearInput();
  stopFrames();
  const total = Math.round((flight.elapsed + flight.penalty) * 1000) / 1000;
  const record = records[course] === null || total < records[course];
  if (record) {
    records[course] = total;
    updateBest();
    saveProfile();
  }
  $('result-time').textContent = clock(total);
  $('result-note').textContent = record
    ? 'New personal best.'
    : 'Personal best  ' + clock(records[course]);
  $('raw-time').textContent = clock(flight.elapsed);
  $('cleared').textContent = flight.hits + ' / ' + GATES.length;
  $('penalty').textContent = '+' + flight.penalty + ' s';
  showPanel('results');
  audio.note(523);
  audio.note(659, 0.13, 0.12);
  audio.note(784, 0.25, 0.23);
}
function mainMenu() {
  closePanel();
  stopFrames();
  clearInput();
  phase = 'menu';
  flight = null;
  $('field').hidden = true;
  $('menu').hidden = false;
  $('location').textContent = 'Main menu';
  $('start').focus({ preventScroll: true });
}
$('start').onclick = startFlight;
$('again').onclick = startFlight;
$('restart').onclick = startFlight;
$('pause').onclick = pauseFlight;
$('resume').onclick = resumeFlight;
for (const b of document.querySelectorAll('[data-menu]')) b.onclick = mainMenu;
routeMap();
refreshDisplay();
updateSettings();
$('start').disabled = true;
$('settings').disabled = true;
function selectCourse(id, definition) {
  useCourse(definition);
  course = id;
  renderer?.loadLandscape();
  $('route-name').textContent = $('hud-course').textContent = COURSE_NAME;
  $('field').setAttribute('aria-label', COURSE_NAME + ' flight');
  $('route-km').textContent = (COURSE_LENGTH / 1000).toFixed(2);
  $('distance').max = COURSE_LENGTH;
  $('route-intro').textContent =
    id === 'copper'
      ? 'Race the river through nine canyon gates.'
      : 'Follow the switchbacks through seven ridge gates.';
  routeMap();
  updateBest();
  saveProfile();
}
loadProfile()
  .then(() => courseMenu(selectCourse, preferences.course))
  .catch((e) => {
    $('course-status').textContent = e.message;
  })
  .finally(() => {
    $('start').disabled = false;
    $('settings').disabled = false;
  });
