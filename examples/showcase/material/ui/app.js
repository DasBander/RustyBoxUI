import { $ } from './chrome.js';
import { bindScene } from './scene.js';
const materials = [
  { name: 'Brushed copper', tint: '#d47543', rough: 24, metal: 90 },
  { name: 'Porcelain', tint: '#ece8db', rough: 16, metal: 2 },
  { name: 'Dark titanium', tint: '#69788a', rough: 38, metal: 96 },
  { name: 'Oxide green', tint: '#26785d', rough: 65, metal: 18 },
];
let selected = 0,
  orbit = 0.36,
  tilt = 0,
  drag = null;
const draw = bindScene($('rb-native-view'), () => {
  const tint = $('#tint').value;
  return {
    primary: [Number($('#rough').value) / 100, Number($('#metal').value) / 100, orbit, tilt],
    secondary: [...[1, 3, 5].map((i) => parseInt(tint.slice(i, i + 2), 16) / 255), 0],
  };
});
function update() {
  const p = materials[selected],
    rough = Number($('#rough').value),
    metal = Number($('#metal').value),
    tint = $('#tint').value;
  $('#rval').textContent = (rough / 100).toFixed(2);
  $('#mval').textContent = (metal / 100).toFixed(2);
  $('#rough').style.setProperty('--fill', rough + '%');
  $('#metal').style.setProperty('--fill', metal + '%');
  $('#hex').textContent = tint.toUpperCase();
  $('#material-state').textContent =
    rough === p.rough && metal === p.metal && tint === p.tint
      ? 'Library preset'
      : 'Modified material';
  draw();
}
function select(index) {
  selected = index;
  const p = materials[index];
  $('#name').textContent = p.name;
  $('#rough').value = p.rough;
  $('#metal').value = p.metal;
  $('#tint').value = p.tint;
  Array.from($('#presets').children).forEach((button, i) =>
    button.setAttribute('aria-pressed', String(index === i))
  );
  update();
}
materials.forEach((m, i) => {
  const b = document.createElement('button'),
    swatch = document.createElement('i');
  swatch.style.setProperty('--swatch', m.tint);
  b.append(swatch, document.createTextNode(m.name));
  b.onclick = () => select(i);
  $('#presets').append(b);
});
['#rough', '#metal', '#tint'].forEach((id) => ($(id).oninput = update));
$('#reset').onclick = () => {
  orbit = 0.36;
  tilt = 0;
  select(selected);
};
const stage = $('#stage');
stage.onpointerdown = (e) => {
  if (e.button !== 0) return;
  drag = [e.clientX, e.clientY];
  stage.setPointerCapture(e.pointerId);
  stage.focus();
};
stage.onpointermove = (e) => {
  if (!drag) return;
  orbit += (e.clientX - drag[0]) * 0.012;
  tilt = Math.max(-1, Math.min(1, tilt + (e.clientY - drag[1]) * 0.015));
  drag = [e.clientX, e.clientY];
  draw();
};
stage.onlostpointercapture = () => {
  drag = null;
};
stage.onkeydown = (e) => {
  if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(e.key)) return;
  e.preventDefault();
  orbit += (Number(e.key === 'ArrowRight') - Number(e.key === 'ArrowLeft')) * 0.12;
  tilt = Math.max(
    -1,
    Math.min(1, tilt + (Number(e.key === 'ArrowDown') - Number(e.key === 'ArrowUp')) * 0.1)
  );
  draw();
};
select(0);
