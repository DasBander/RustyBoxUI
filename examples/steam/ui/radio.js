import { escapeHTML as esc } from './sdk/components.js';
import { gameById, profile, change, record, playStats } from './profile.js';
import { glyph } from './views.js';
const $ = (selector) => document.querySelector(selector);

export function createRadio({ showDialog, dialogHead, render }) {
  let session = null,
    starting = false,
    finishing = null;
  function strength() {
    const target = session.pack.channels[session.round].frequency;
    return Math.round(100 / (1 + Math.abs(target - session.dial) / 23));
  }
  function tune(value) {
    if (!session || finishing) return;
    session.dial = Math.max(880, Math.min(1080, Number(value)));
    const frequency = `${(session.dial / 10).toFixed(1)} MHz`;
    $('#dial').value = session.dial;
    $('#dial').setAttribute('aria-valuetext', frequency);
    $('#frequency').textContent = frequency;
    $('#strength').value = strength();
    $('#strength-value').textContent = `${strength()}%`;
    $('#radio-feedback').textContent = strength() >= 96 ? 'Ready to lock.' : '';
  }
  function show() {
    const g = gameById.get(session.id),
      channel = session.pack.channels[session.round];
    showDialog(
      /* HTML */ `${dialogHead('Radio check')}
        <div class="radio-screen">
          <div class="radio-label">
            <span>${glyph('radio', 18)} ${esc(g.title)}</span
            ><span>Channel ${session.round + 1} / 3</span>
          </div>
          <h3>${esc(channel.station)}</h3>
          <output id="frequency"></output> <label for="dial">Tuning frequency</label
          ><input
            id="dial"
            type="range"
            min="880"
            max="1080"
            step="1"
            aria-describedby="dial-hint"
          />
          <div class="range-labels"><span>88.0 MHz</span><span>108.0 MHz</span></div>
          <div class="strength">
            <label for="strength">Signal strength</label><output id="strength-value"></output>
          </div>
          <progress id="strength" max="100"></progress>
        </div>
        <p id="dial-hint">Adjust the dial until signal strength reaches 96% or higher.</p>
        <div class="dialog-actions">
          <button class="green" data-action="lock">${glyph('radio', 17)} Lock channel</button
          ><span id="radio-feedback" role="status"></span>
        </div>`,
      'radio'
    );
    tune(session.dial);
    $('#dial').focus({ preventScroll: true });
  }
  async function start(id) {
    if (starting || session || finishing) return;
    starting = true;
    try {
      if (!profile.packs[id]) throw new Error('Install this demo first.');
      await change((next) => {
        const stats = playStats(next.stats[id]);
        stats.launches++;
        stats.last = Date.now();
        next.stats[id] = stats;
        record(next, 'play', id);
      });
      session = { id, pack: profile.packs[id], round: 0, dial: 980, misses: 0, start: Date.now() };
      render();
      show();
    } finally {
      starting = false;
    }
  }
  async function finish(won = false) {
    if (finishing) return finishing;
    if (!session) return;
    const current = session;
    const elapsed = Math.max(1, Math.round((Date.now() - current.start) / 1000));
    finishing = change((next) => {
      const stats = playStats(next.stats[current.id]);
      stats.seconds += elapsed;
      if (won) {
        stats.completions++;
        stats.clean ||= current.misses === 0;
        record(next, 'finish', current.id, { seconds: elapsed });
      }
      next.stats[current.id] = stats;
    })
      .then(() => {
        session = null;
        render();
        if (won && $('#overlay').dialog.open && $('#overlay').dialog.dataset.kind === 'radio') {
          showDialog(
            /* HTML */ `${dialogHead('Transmission complete')}
              <div class="result-emblem">${glyph('radio', 44)}</div>
              <h3 class="result-title">All three channels locked</h3>
              <p>
                ${current.misses === 0
                  ? 'Clean sweep. Every station answered on the first attempt.'
                  : `${current.misses} missed ${current.misses === 1 ? 'lock' : 'locks'} in this session.`}
              </p>
              <button class="secondary" data-dismiss>Return to library</button>`,
            'result'
          );
        }
      })
      .catch((error) => {
        if (!$('#overlay').dialog.open) session = null;
        else if ($('#radio-feedback'))
          $('#radio-feedback').textContent = 'Could not save. Lock the channel again to retry.';
        throw error;
      })
      .finally(() => {
        finishing = null;
      });
    return finishing;
  }
  async function lock() {
    if (!session || finishing) return;
    if (strength() < 96) {
      session.misses++;
      $('#radio-feedback').textContent = 'Weak signal. Keep tuning.';
      return;
    }
    if (session.round + 1 === session.pack.channels.length) await finish(true);
    else {
      session.round++;
      session.dial = 980;
      show();
      $('#radio-feedback').textContent = 'Previous channel locked.';
    }
  }
  return { start, finish, lock, tune };
}
