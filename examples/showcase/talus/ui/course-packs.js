import { ContentPacks } from './sdk/content-packs.js';
const id = 'switchback';

// The sample's feed travels with the game. A publisher can serve these same files over HTTPS.
export async function courseMenu(selectCourse, preferred) {
  const button = document.getElementById('course-install');
  const choice = document.getElementById('course-choice');
  const status = document.getElementById('course-status');
  const response = await fetch('./packs/client.json');
  if (!response.ok) throw new Error('Course catalog unavailable.');
  const store = new ContentPacks('talus', await response.json());
  let installed = Boolean(await store.installed(id));
  let activeDownload;
  const update = () => {
    choice.options[1].disabled = !installed;
    button.disabled = false;
    button.textContent = activeDownload ? 'Pause' : installed ? 'Remove' : 'Install';
  };
  const select = async (name) => {
    const data =
      name === id ? JSON.parse(await (await store.read(id, 'course.json')).text()) : undefined;
    selectCourse(name, data);
    choice.value = name;
  };
  choice.onchange = () =>
    select(choice.value).catch((e) => {
      status.textContent = e.message;
    });
  button.onclick = async () => {
    if (activeDownload) {
      activeDownload.abort();
      return;
    }
    button.disabled = true;
    choice.disabled = true;
    try {
      if (installed) {
        await store.remove(id);
        installed = false;
        await select('copper');
        status.textContent = 'Pack removed.';
      } else {
        activeDownload = new AbortController();
        update();
        await store.install(id, './packs/switchback.rbcontent', {
          signal: activeDownload.signal,
          onProgress: ({ verified, total }) => {
            status.textContent = 'Installing ' + Math.round((100 * verified) / (total || 1)) + '%';
          },
        });
        installed = true;
        await select(id);
        status.textContent = 'Ready to fly.';
      }
    } catch (error) {
      status.textContent =
        error.name === 'AbortError' ? 'Paused. Install again to resume.' : error.message;
    } finally {
      activeDownload = null;
      choice.disabled = false;
      update();
    }
  };
  if (installed && preferred === id) await select(id);
  update();
}
