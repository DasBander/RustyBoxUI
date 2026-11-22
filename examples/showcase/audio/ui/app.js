import { $, ready, showError } from './chrome.js';
import { wavInfo, peaks, timestamp } from './wave.js';
const canvas = $('#wave'),
  ctx = canvas.getContext('2d'),
  transport = $('#play');
const style = getComputedStyle(canvas);
const ink = Object.fromEntries(
  [
    'bg',
    'grid',
    'grid-minor',
    'baseline',
    'ruler',
    'signal',
    'selection',
    'selected-signal',
    'handle',
    'cursor',
  ].map((role) => [role, style.getPropertyValue('--wave-' + role).trim()])
);
const rulerFont = '500 11px ' + style.getPropertyValue('--mono').trim();
let clip = null,
  channels = [],
  start = 0,
  end = 0,
  cursor = 0,
  loop = true,
  context = null,
  source = null,
  sourceTime = 0;
let bounds = null,
  drag = null,
  sequence = 0,
  playRequest = 0;
const clamp = (n, min, max) => Math.max(min, Math.min(max, n));
function position() {
  if (!source) return cursor;
  const elapsed = cursor + context.currentTime - sourceTime;
  return loop
    ? start + ((((elapsed - start) % (end - start)) + (end - start)) % (end - start))
    : Math.min(end, elapsed);
}
function discard() {
  if (!source) return;
  source.onended = null;
  source.stop();
  source.disconnect();
  source = null;
}
function playAt(at) {
  discard();
  cursor = at;
  sourceTime = context.currentTime;
  const node = context.createBufferSource();
  node.buffer = clip;
  node.loop = loop;
  node.loopStart = start;
  node.loopEnd = end;
  node.connect(context.destination);
  source = node;
  node.onended = () => {
    if (source === node) {
      node.disconnect();
      source = null;
      cursor = end;
      paint();
    }
  };
  node.start(0, at);
  if (!loop) node.stop(context.currentTime + end - at);
}
async function toggle() {
  if (!clip) return;
  const ticket = ++playRequest;
  if (source) {
    const at = position();
    discard();
    cursor = at;
    paint();
    return;
  }
  context ??= new AudioContext();
  await context.resume();
  if (ticket !== playRequest) return;
  playAt(cursor >= start && cursor < end ? cursor : start);
  paint();
}
function setRange(a, b) {
  if (!clip || !Number.isFinite(a) || !Number.isFinite(b)) return;
  const at = position(),
    min = Math.min(0.01, clip.duration);
  start = clamp(a, 0, clip.duration - min);
  end = clamp(b, start + min, clip.duration);
  if (source) {
    if (at < start || at >= end || !loop) playAt(at >= start && at < end ? at : start);
    else {
      source.loopStart = start;
      source.loopEnd = end;
      sourceTime = context.currentTime;
      cursor = at;
    }
  } else cursor = clamp(cursor, start, end);
  $('#start').value = start.toFixed(3);
  $('#end').value = end.toFixed(3);
  $('#length').textContent = (end - start).toFixed(3) + ' s';
  paint();
}
function paint() {
  if (!bounds) return;
  const { w, h } = bounds;
  ctx.fillStyle = ink.bg;
  ctx.fillRect(0, 0, w, h);
  if (!clip || h < 45 || w < 20) return;
  const left = 30,
    right = w - 12,
    span = right - left,
    ruler = 30,
    bottom = h - 13;
  const x = (t) => left + (t / clip.duration) * span;
  ctx.font = rulerFont;
  ctx.textBaseline = 'middle';
  ctx.fillStyle = ink.selection;
  ctx.fillRect(x(start), ruler, x(end) - x(start), bottom - ruler);
  const tick = 10 ** Math.ceil(Math.log10(clip.duration / Math.max(1, span / 65)));
  const step = clip.duration / tick < 4 ? tick / 5 : tick;
  for (let t = 0; t < clip.duration + 0.000001; t += step / 2) {
    const px = Math.round(x(t)) + 0.5,
      major = Math.abs(t / step - Math.round(t / step)) < 0.001;
    ctx.fillStyle = major ? ink.grid : ink['grid-minor'];
    ctx.fillRect(px, major ? ruler - 6 : ruler - 3, 1, bottom - ruler + 4);
    if (major) {
      ctx.fillStyle = ink.ruler;
      const label = Number(t.toFixed(2)) + 's';
      ctx.fillText(label, Math.min(px + 3, w - ctx.measureText(label).width - 3), 12);
    }
  }
  const track = (bottom - ruler) / channels.length;
  channels.forEach((data, index) => {
    const center = ruler + track * (index + 0.5);
    ctx.fillStyle = ink.baseline;
    ctx.fillRect(left, Math.floor(center), span, 1);
    ctx.fillStyle = ink.ruler;
    ctx.fillText(channels.length === 1 ? 'M' : index === 0 ? 'L' : 'R', 10, center);
    for (let p = 0; p < span; p++) {
      const from = Math.floor(((p / span) * data.length) / 2),
        to = Math.min(
          data.length / 2,
          Math.max(from + 1, Math.floor((((p + 1) / span) * data.length) / 2))
        );
      let low = 0,
        high = 0;
      for (let j = from; j < to; j++) {
        low = Math.min(low, data[2 * j]);
        high = Math.max(high, data[2 * j + 1]);
      }
      const selected = p + left >= x(start) && p + left <= x(end);
      ctx.fillStyle = selected ? ink['selected-signal'] : ink.signal;
      ctx.fillRect(
        left + p,
        center - high * track * 0.54,
        1,
        Math.max(1, (high - low) * track * 0.54)
      );
    }
    if (index < channels.length - 1) {
      ctx.fillStyle = ink.grid;
      ctx.fillRect(0, ruler + track, w, 1);
    }
  });
  ctx.fillStyle = ink.handle;
  ctx.fillRect(x(start), ruler, x(end) - x(start), 2);
  for (const t of [start, end]) {
    ctx.fillRect(Math.round(x(t)), ruler, 1, bottom - ruler);
    ctx.fillRect(x(t) - 3, ruler, 7, 10);
  }
  const at = position();
  ctx.fillStyle = ink.cursor;
  ctx.fillRect(Math.round(x(at)), ruler - 3, 1, bottom - ruler + 4);
  $('#clock').textContent = timestamp(at);
  const label = source ? 'Pause' : 'Play';
  if (transport.textContent !== label) {
    transport.textContent = label;
    transport.setAttribute('aria-label', source ? 'Pause playback' : 'Play selection');
  }
}
document.fonts.load(rulerFont).then(paint).catch(showError);
new ResizeObserver(() => {
  const r = canvas.getBoundingClientRect(),
    dpr = Math.min(2, devicePixelRatio);
  canvas.width = Math.max(1, Math.round(r.width * dpr));
  canvas.height = Math.max(1, Math.round(r.height * dpr));
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  bounds = { w: r.width, h: r.height };
  paint();
}).observe(canvas);
function frame() {
  if (source) paint();
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
function timeAt(e) {
  return clamp(
    ((e.clientX - canvas.getBoundingClientRect().left - 30) / (bounds.w - 42)) * clip.duration,
    0,
    clip.duration
  );
}
canvas.onpointerdown = (e) => {
  if (!clip || e.button !== 0) return;
  const t = timeAt(e),
    zone = (8 * clip.duration) / (bounds.w - 42);
  drag = {
    anchor: t,
    mode: Math.abs(t - start) < zone ? 'start' : Math.abs(t - end) < zone ? 'end' : 'new',
    moved: false,
    x: e.clientX,
  };
  canvas.setPointerCapture(e.pointerId);
};
canvas.onpointermove = (e) => {
  if (!drag) return;
  const t = timeAt(e);
  if (Math.abs(e.clientX - drag.x) < 2 && !drag.moved) return;
  drag.moved = true;
  if (drag.mode === 'start') setRange(Math.min(t, end - 0.01), end);
  else if (drag.mode === 'end') setRange(start, Math.max(t, start + 0.01));
  else setRange(Math.min(t, drag.anchor), Math.max(t, drag.anchor));
};
canvas.onpointerup = (e) => {
  if (drag && !drag.moved && drag.mode === 'new') {
    const at = clamp(timeAt(e), start, end);
    if (source) playAt(at >= end ? start : at);
    else cursor = at;
    paint();
  }
  drag = null;
};
canvas.onlostpointercapture = () => {
  drag = null;
};
async function load(bytes, name, token, isDemo) {
  const info = wavInfo(bytes);
  const audio = await new OfflineAudioContext(1, 1, info.rate).decodeAudioData(bytes);
  if (token !== sequence) return;
  const envelope = peaks(audio);
  ++playRequest;
  discard();
  clip = audio;
  channels = envelope;
  cursor = 0;
  $('#filename').textContent = name;
  $('#channels').textContent =
    audio.numberOfChannels === 1
      ? 'Mono'
      : audio.numberOfChannels === 2
        ? 'Stereo'
        : audio.numberOfChannels + ' channels · first two displayed';
  $('#format').textContent =
    'WAV  /  ' +
    info.rate.toLocaleString('en-US') +
    ' Hz  /  ' +
    info.bits +
    ' bit  /  ' +
    audio.duration.toFixed(2) +
    ' s';
  $('#start').max = audio.duration;
  $('#end').max = audio.duration;
  setRange(isDemo ? 4 : 0, isDemo ? 12 : audio.duration);
  $('#notice').textContent = 'Drop a WAV file to open it';
  ready();
}
async function openFile(file) {
  if (!file) return;
  const token = ++sequence;
  try {
    if (file.size > 64 * 1024 * 1024) throw new Error('Choose a WAV file under 64 MiB.');
    await load(await file.arrayBuffer(), file.name, token, false);
  } catch (e) {
    if (token === sequence) $('#notice').textContent = e.message;
  }
}
$('#open').onclick = () => $('#file').click();
$('#file').onchange = (e) => {
  openFile(e.target.files[0]);
  e.target.value = '';
};
document.addEventListener('dragover', (e) => e.preventDefault());
document.addEventListener('drop', (e) => {
  e.preventDefault();
  openFile(e.dataTransfer.files[0]);
});
transport.onclick = () => toggle().catch(showError);
$('#stop').onclick = () => {
  ++playRequest;
  discard();
  cursor = start;
  paint();
};
$('#loop').onclick = () => {
  const at = position();
  loop = !loop;
  $('#loop').setAttribute('aria-pressed', String(loop));
  $('#loop').textContent = loop ? 'Loop on' : 'Loop off';
  if (source) playAt(at >= end ? start : at);
  paint();
};
$('#start').onchange = () => setRange(Math.min(Number($('#start').value), end - 0.01), end);
$('#end').onchange = () => setRange(start, Number($('#end').value));
document.addEventListener('keydown', (e) => {
  if (e.code === 'Space' && !e.repeat && !['INPUT', 'BUTTON'].includes(e.target.tagName)) {
    e.preventDefault();
    toggle().catch(showError);
  }
});
const initial = ++sequence;
fetch('breakwater.wav')
  .then((r) => {
    if (!r.ok) throw new Error('Bundled WAV could not be opened.');
    return r.arrayBuffer();
  })
  .then((b) => load(b, 'Breakwater', initial, true))
  .catch(showError);
