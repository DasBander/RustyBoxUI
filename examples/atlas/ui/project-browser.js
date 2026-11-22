// The native side retains filesystem grants. The browser receives labels, opaque IDs and thumbnails.
export function projectBrowser(commands, onSelect, report) {
  const tree = document.querySelector('#folders');
  const table = document.querySelector('#files');
  const thumbnails = new Map();
  let folder = '*',
    query = '',
    assets = [],
    loading = false,
    frame = 0;
  function scheduleThumbnails() {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      loadVisible().catch(report);
    });
  }
  async function loadVisible() {
    if (loading || table.closest('[hidden]')) return;
    const images = Array.from(table.querySelectorAll('img[data-asset]'));
    const ids = [...new Set(images.map((image) => image.dataset.asset))]
      .filter((id) => !thumbnails.has(id))
      .slice(0, 16);
    if (!ids.length) return;
    loading = true;
    try {
      for (const { id, url } of await commands.thumbnails({ ids })) thumbnails.set(id, url);
      for (const image of table.querySelectorAll('img[data-asset]')) {
        const url = thumbnails.get(image.dataset.asset);
        if (url && image.getAttribute('src') !== url) image.src = url;
      }
      if (thumbnails.size > 384) {
        // Keep the displayed rows when evicting previous scroll positions.
        const visible = new Set(images.map((image) => image.dataset.asset));
        for (const id of thumbnails.keys()) if (!visible.has(id)) thumbnails.delete(id);
      }
      scheduleThumbnails();
    } finally {
      loading = false;
    }
  }
  table.columns = [
    {
      key: 'title',
      label: 'Texture',
      render(title, asset) {
        const cell = document.createElement('span');
        cell.className = 'file-cell';
        const image = document.createElement('img');
        image.alt = '';
        const url = asset.thumbnail || thumbnails.get(asset.id);
        if (url) image.src = url;
        if (!asset.thumbnail) {
          image.dataset.asset = asset.id;
          scheduleThumbnails();
        }
        const name = document.createElement('span');
        name.textContent = title;
        cell.append(image, name);
        return cell;
      },
    },
    {
      key: 'location',
      label: 'Folder',
      render: (name, asset) => (asset.thumbnail ? 'Bundled plates' : name || 'Project root'),
    },
    { key: 'width', label: 'Dimensions', render: (width, asset) => width + ' × ' + asset.height },
  ];
  function directoryNodes(rows) {
    const root = { children: new Map() };
    for (const row of rows) {
      if (row.thumbnail) continue;
      let parent = root,
        path = '';
      for (const name of row.location.split('/').filter(Boolean)) {
        path += '/' + name;
        if (!parent.children.has(name))
          parent.children.set(name, { id: path, label: name, children: new Map() });
        parent = parent.children.get(name);
      }
    }
    const children = (node) =>
      Array.from(node.children.values())
        .sort((a, b) => a.label.localeCompare(b.label))
        .map((child) => ({ id: child.id, label: child.label, children: children(child) }));
    return [
      { id: '*', label: 'All assets' },
      { id: 'plates', label: 'Bundled plates' },
      { id: '/', label: 'Project files', children: children(root) },
    ];
  }
  function filter() {
    table.rows = assets.filter((a) => {
      const path = '/' + a.location;
      const inside =
        folder === '*' ||
        (a.thumbnail
          ? folder === 'plates'
          : folder === '/' || path === folder || path.startsWith(folder + '/'));
      return (
        inside &&
        (!query || (a.fileName + ' ' + a.location + ' ' + a.title).toLowerCase().includes(query))
      );
    });
    scheduleThumbnails();
  }
  tree.addEventListener('selection-change', (e) => {
    folder = e.detail.id;
    filter();
  });
  table.addEventListener('selection-change', (e) => onSelect(e.detail.id).catch(report));
  return {
    update(rows, reset) {
      if (reset) {
        folder = '*';
        thumbnails.clear();
      }
      assets = rows;
      tree.items = directoryNodes(rows);
      filter();
    },
    search(text) {
      query = text;
      filter();
    },
    select(id) {
      table.selectId(id);
    },
  };
}
