import { $, rustybox, showError } from './chrome.js';
import { bindScene } from './scene.js';
import { advance, curvature, lapLength, newLap } from './drive.js';
let car = newLap(),
  mode = 'grid',
  best = null,
  last = 0;
const keys = new Set(),
  menu = $('#menu');
const format = (seconds) => {
  const ms = Math.round(seconds * 1000);
  return (
    String(Math.floor(ms / 60000)).padStart(2, '0') +
    ':' +
    String(Math.floor(ms / 1000) % 60).padStart(2, '0') +
    '.' +
    String(ms % 1000).padStart(3, '0')
  );
};
const draw = bindScene($('rb-native-view'), () => ({
  primary: [car.distance, car.lane, curvature(car.distance), Math.sin(car.distance * 0.004)],
  secondary: [car.steer, 0, 0, 0],
}));
function paint() {
  $('#speed').textContent = String(Math.round(car.speed * 3.6)).padStart(3, '0');
  $('#time').textContent = format(car.time);
  $('#best').textContent = best === null ? '—' : format(best);
  $('#meters').textContent = Math.floor(car.distance).toLocaleString('en-US') + ' / 1,800 m';
  $('#distance').style.width = (car.distance / lapLength) * 100 + '%';
  $('#rev').style.width = Math.min(100, 12 + (car.speed / 70) * 88) + '%';
  draw();
}
function dialog(finished) {
  mode = finished ? 'finished' : 'paused';
  keys.clear();
  $('#pause').disabled = true;
  $('#menu-title').textContent = finished ? 'Lap complete' : 'Paused';
  $('#result').textContent = finished
    ? 'Lap time  ' + format(car.time)
    : 'Lap time  ' + format(car.time) + '   /   ' + Math.round(car.distance) + ' m';
  $('#resume').hidden = finished;
  menu.showModal();
  (finished ? $('#restart') : $('#resume')).focus();
}
function drive(reset) {
  if (reset) car = newLap();
  if (menu.open) menu.close();
  mode = 'driving';
  $('#brief').hidden = true;
  $('#pause').disabled = false;
  keys.clear();
  last = performance.now();
  $('#track').focus();
  paint();
}
function tick(now) {
  if (mode === 'driving') {
    advance(car, keys, $('#assist').checked, Math.min(0.08, Math.max(0, (now - last) / 1000)));
    if (car.distance >= lapLength) {
      if (best === null || car.time < best) {
        best = car.time;
        rustybox.invoke('storage.set', { key: 'bestLap', value: best }).catch(showError);
      }
      dialog(true);
    }
    paint();
  }
  last = now;
  requestAnimationFrame(tick);
}
$('#start').onclick = () => drive(true);
$('#restart').onclick = () => drive(true);
$('#resume').onclick = () => drive(false);
$('#pause').onclick = () => {
  if (mode === 'driving') dialog(false);
};
menu.addEventListener('cancel', (event) => {
  event.preventDefault();
  if (mode === 'paused') drive(false);
});
const drivingKeys = [
  'KeyW',
  'KeyA',
  'KeyS',
  'KeyD',
  'ArrowUp',
  'ArrowLeft',
  'ArrowDown',
  'ArrowRight',
];
document.addEventListener('keydown', (event) => {
  if (event.code === 'Escape' && !event.repeat && mode === 'driving') {
    event.preventDefault();
    dialog(false);
  } else if (mode === 'driving' && drivingKeys.includes(event.code)) {
    event.preventDefault();
    keys.add(event.code);
  }
});
document.addEventListener('keyup', (event) => keys.delete(event.code));
window.addEventListener('blur', () => {
  keys.clear();
  if (mode === 'driving') dialog(false);
});
$('#assist').onchange = () =>
  rustybox.invoke('storage.set', { key: 'assist', value: $('#assist').checked }).catch(showError);
Promise.all(['bestLap', 'assist'].map((key) => rustybox.invoke('storage.get', { key })))
  .then(([time, assist]) => {
    if (typeof time === 'number' && time > 0) best = time;
    if (typeof assist === 'boolean') $('#assist').checked = assist;
    paint();
  })
  .catch(showError);
requestAnimationFrame(tick);
