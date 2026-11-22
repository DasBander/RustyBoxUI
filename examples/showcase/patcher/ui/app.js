import { $, rustybox, ready, showError } from './chrome.js';
const fmt = (n) =>
  n < 1024
    ? n + ' B'
    : n < 1048576
      ? (n / 1024).toFixed(0) + ' KiB'
      : (n / 1048576).toFixed(2) + ' MiB';
const states = {
  checking: ['Checking files…', 'Checking the installed content against the bundled package.'],
  ready: [
    'Update available',
    'Install the station artwork, transmission loop, and tuning records.',
  ],
  running: ['Installing update', 'Writing and checking each chunk of the local package.'],
  paused: [
    'Installation paused',
    'Your verified files are saved. Resume from the last complete chunk.',
  ],
  complete: ['Up to date', 'All three files match the bundled release.'],
  error: ['Installation interrupted', 'Verify the local files or retry the update.'],
};
let state = null,
  inFlight = false,
  generation = 0,
  lastSnapshot = '';
let desktopSnapshot;
function trayOptions(running) {
  return {
    tooltip: 'Relay game updates',
    keepRunning: true,
    menu: [
      { id: 'transfer', label: running ? 'Pause update' : 'Resume update' },
      { id: 'verify', label: 'Verify files', enabled: !running },
    ],
  };
}
async function updateDesktop(s, previous) {
  if (!$('#tray').checked) return;
  if (previous !== s.state) await rustybox.invoke('shell.tray', trayOptions(s.state === 'running'));
  const mode =
    { running: 'normal', paused: 'paused', error: 'error', checking: 'indeterminate' }[s.state] ||
    'none';
  await rustybox.invoke('shell.progress', {
    state: mode,
    completed: s.verified,
    total: Math.max(1, s.total),
  });
  if (s.state === 'complete' && previous === 'running') {
    await rustybox.invoke('shell.notify', {
      title: 'Relay update complete',
      body: 'The Last Signal is ready to play.',
    });
  }
}
$('#tray-setting').hidden = !rustybox.isNative;
$('#tray').onchange = async () => {
  const control = $('#tray');
  control.disabled = true;
  try {
    await rustybox.invoke(
      'shell.tray',
      control.checked ? trayOptions(state === 'running') : { enabled: false }
    );
    if (control.checked && desktopSnapshot) await updateDesktop(desktopSnapshot, state);
    if (!control.checked) await rustybox.invoke('shell.progress', { state: 'none' });
  } catch (error) {
    control.checked = false;
    await rustybox.invoke('shell.tray', { enabled: false }).catch(() => {});
    showError(error);
  } finally {
    control.disabled = false;
  }
};
rustybox.addEventListener('shell-action', ({ detail }) => {
  if (detail.id === 'transfer') command(state === 'running' ? 'pause' : 'start');
  if (detail.id === 'verify') command('verify');
});
function draw(s) {
  const snapshot = JSON.stringify(s);
  if (snapshot === lastSnapshot) return;
  lastSnapshot = snapshot;
  desktopSnapshot = s;
  updateDesktop(s, state).catch(showError);
  state = s.state;
  $('.overview').dataset.state = state;
  $('#status').textContent = states[state][0];
  $('#summary').textContent = s.error || states[state][1];
  const pct = Math.floor((s.verified / s.total) * 100);
  $('#bar').value = pct;
  $('#percent').textContent = pct + '%';
  const verified = fmt(s.verified);
  const total = fmt(s.total);
  const remaining = s.total - s.verified;
  // A tiny pending file can round both byte counts to the same number of MiB.
  $('#size').textContent =
    remaining > 0 && verified === total
      ? total + ' total · ' + fmt(remaining) + ' remaining'
      : verified + ' / ' + total + ' verified';
  $('#action').textContent =
    {
      running: 'Pause update',
      paused: 'Resume update',
      complete: 'Installed',
      error: 'Retry update',
    }[state] || 'Install update';
  $('#action').disabled = ['checking', 'complete'].includes(state);
  $('#verify').disabled = state === 'checking' || state === 'running';
  const list = $('#files');
  list.replaceChildren();
  s.files.forEach((f) => {
    const row = document.createElement('div'),
      title = document.createElement('strong'),
      status = document.createElement('small'),
      progress = document.createElement('span');
    row.className = 'file';
    title.textContent = f.name;
    progress.textContent = fmt(f.bytes);
    status.textContent =
      f.verified === f.bytes ? 'Verified' : f.verified ? 'Partial · ' + fmt(f.verified) : 'Pending';
    if (f.verified === f.bytes) status.className = 'verified';
    else if (f.verified) status.className = 'partial';
    row.append(title, status, progress);
    list.append(row);
  });
  ready();
}
async function poll() {
  const token = generation;
  try {
    if (!inFlight) {
      const s = await rustybox.invoke('host.patch', { action: 'status' });
      if (token === generation) draw(s);
    }
  } catch (e) {
    showError(e);
  }
  setTimeout(poll, state === 'running' ? 125 : 400);
}
async function command(action) {
  if (inFlight) return;
  inFlight = true;
  ++generation;
  try {
    await rustybox.invoke('host.patch', { action });
    const s = await rustybox.invoke('host.patch', { action: 'status' });
    draw(s);
    if (action === 'verify' && s.state === 'complete')
      $('#summary').textContent = 'Integrity check passed. All installed files match the package.';
  } catch (e) {
    showError(e);
  } finally {
    inFlight = false;
  }
}
$('#action').onclick = () => command(state === 'running' ? 'pause' : 'start');
$('#verify').onclick = () => command('verify');
poll();
