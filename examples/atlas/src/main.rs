#![windows_subsystem = "windows"]
#[rustfmt::skip]
mod commands;
mod image;
mod project;
use commands::*;
use project::{LocalImage, Saved, Update};
use rustybox_sdk::{
    Action, App, Job, Result, View, Window, Windows,
    files::{self, FileGrant},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

struct Atlas {
    content: rustybox_assets::Assets,
    bundled: Vec<Asset>,
    saved: Saved,
    positions: HashMap<String, usize>,
    thumbs: HashMap<String, String>,
    pixels: Option<(String, image::Pixels)>,
    job: Option<Job<Update, ()>>,
    restore_after_scan: Option<String>,
    skipped: u32,
    generation: u32,
    revision: u64,
    last_seen: HashMap<isize, u64>,
    last_update: Instant,
    message: String,
    windows: Windows,
    web_root: PathBuf,
    profile: PathBuf,
    hidden: bool,
    dirty: bool,
}
impl Atlas {
    fn new(web_root: PathBuf, profile: PathBuf, windows: Windows, hidden: bool) -> Result<Self> {
        std::fs::create_dir_all(profile.join("thumbnails")).map_err(|e| e.to_string())?;
        let content = rustybox_assets::Assets::open(&web_root)?;
        let mut bundled = vec![];
        for (id, title, location, credit) in [
            (
                "lena-delta",
                "Lena River Delta",
                "Sakha, Russia",
                "USGS EROS · Landsat 7",
            ),
            (
                "namib",
                "Namib Sand Sea",
                "Namibia",
                "NASA Earth Observatory · USGS",
            ),
            (
                "vatnajokull",
                "Vatnajökull",
                "Iceland",
                "USGS EROS · Landsat 7",
            ),
        ] {
            let name = format!("{id}.jpg");
            let url = format!("plates/{name}");
            let bytes = content.read(&url)?.ok_or("Missing bundled plate")?;
            let (width, height) = image::dimensions(&bytes)?;
            bundled.push(Asset {
                id: id.into(),
                title: title.into(),
                file_name: name,
                location: location.into(),
                credit: credit.into(),
                thumbnail: Some(url),
                width,
                height,
                bytes: bytes.len() as u32,
            });
        }
        let mut saved = Saved::load(&profile.join("project.json"))?;
        let granted: Vec<_> = saved
            .singles
            .iter()
            .filter_map(|p| p.canonicalize().ok())
            .collect();
        let folder = saved.folder.as_ref().and_then(|p| p.canonicalize().ok());
        saved.images.retain(|item| {
            item.id.len() == 69
                && item.id.starts_with("file-")
                && item.id[5..].bytes().all(|c| c.is_ascii_hexdigit())
                && (folder
                    .as_ref()
                    .is_some_and(|root| item.path.starts_with(root))
                    || granted.contains(&item.path))
        });
        if !saved.images.iter().any(|i| i.id == saved.selected)
            && !bundled.iter().any(|i| i.id == saved.selected)
        {
            saved.selected = bundled[0].id.clone();
        }
        let positions = saved
            .images
            .iter()
            .enumerate()
            .map(|(i, item)| (item.id.clone(), i))
            .collect();
        Ok(Self {
            content,
            bundled,
            saved,
            positions,
            thumbs: HashMap::new(),
            pixels: None,
            job: None,
            restore_after_scan: None,
            skipped: 0,
            generation: 1,
            revision: 1,
            last_seen: HashMap::new(),
            last_update: Instant::now(),
            message: String::new(),
            windows,
            web_root,
            profile,
            hidden,
            dirty: false,
        })
    }
    fn window(&self, title: &str) -> Result<Window> {
        let mut config = Window::new(title, self.web_root.clone(), self.profile.clone())?;
        config.composed = true;
        config.hidden = self.hidden;
        Ok(config)
    }
    fn save(&mut self) -> Result<()> {
        if self.dirty {
            self.saved.save(&self.profile.join("project.json"))?;
            self.dirty = false;
        }
        Ok(())
    }
    fn added(&mut self, item: LocalImage, thumb: String) {
        if !self.positions.contains_key(&item.id) {
            self.positions
                .insert(item.id.clone(), self.saved.images.len());
            self.saved.images.push(item.clone());
            self.dirty = true;
        }
        // The cache contains thumbnails of user-imported files, outside the distributed application.
        let _ = std::fs::write(
            self.profile.join("thumbnails").join(&item.id),
            thumb.as_bytes(),
        );
        if self.thumbs.len() >= 256 {
            self.thumbs.clear();
        }
        if self.restore_after_scan.as_deref() == Some(&item.id) {
            self.saved.selected = item.id.clone();
            self.restore_after_scan = None;
        }
        self.thumbs.insert(item.id, thumb);
    }
    fn open_folder(&mut self, folder: FileGrant) -> Result<()> {
        let job = project::scan(folder.clone())?;
        self.job = Some(job);
        self.skipped = 0;
        self.message.clear();
        self.restore_after_scan = (self.saved.folder.as_deref() == Some(folder.path())
            && self.positions.contains_key(&self.saved.selected))
        .then(|| self.saved.selected.clone());
        self.generation = self.generation.wrapping_add(1);
        self.saved.folder = Some(folder.path().into());
        self.saved.images.clear();
        self.saved.singles.clear();
        self.positions.clear();
        self.thumbs.clear();
        self.saved.selected = self.bundled[0].id.clone();
        self.pixels = None;
        self.dirty = true;
        self.revision += 1;
        self.save()
    }
    fn import_file(&mut self, view: &View, grant: FileGrant) -> Result<Asset> {
        let (entry, thumb) = project::inspect(grant.path())?;
        let id = entry.id.clone();
        if !self.saved.singles.contains(&entry.path) {
            self.saved.singles.push(entry.path.clone());
        }
        self.added(entry, thumb);
        self.revision += 1;
        self.select_asset(view, Selection { id })
    }
    fn paint(&mut self, view: &View, id: &str, slot: u32) -> Result<Asset> {
        let bundled = self.bundled.iter().find(|a| a.id == id);
        let local = self
            .positions
            .get(id)
            .and_then(|i| self.saved.images.get(*i));
        let asset = bundled
            .cloned()
            .or_else(|| local.map(|item| item.metadata(self.saved.folder.as_deref())))
            .ok_or("Unknown asset")?;
        if !self.pixels.as_ref().is_some_and(|p| p.0 == id) {
            let data = if let Some(local) = local {
                let path = local.path.canonicalize().map_err(|e| e.to_string())?;
                if !self
                    .saved
                    .folder
                    .as_ref()
                    .is_some_and(|root| path.starts_with(root))
                    && !self.saved.singles.contains(&path)
                {
                    return Err("Image is outside the granted locations".into());
                }
                project::read(&path)?
            } else {
                self.content
                    .read(asset.thumbnail.as_deref().ok_or("Missing plate")?)?
                    .ok_or("Missing plate")?
            };
            self.pixels = Some((id.to_owned(), image::decode(&data)?));
        }
        self.pixels.as_ref().unwrap().1.display(view, slot)?;
        Ok(asset)
    }
    fn update(&mut self, view: &View) -> Result<()> {
        if self.last_update.elapsed() >= Duration::from_millis(75) {
            self.last_update = Instant::now();
            if let Some(job) = self.job.as_ref() {
                let mut updates: Vec<_> = job.progress().collect();
                let result = job.try_finish();
                if result.is_some() {
                    updates.extend(job.progress());
                }
                if !updates.is_empty() {
                    self.revision += 1;
                }
                for update in updates {
                    match update {
                        Update::Image(item, thumb) => self.added(item, thumb),
                        Update::Skipped => self.skipped += 1,
                    }
                }
                if let Some(result) = result {
                    self.job = None;
                    self.restore_after_scan = None;
                    self.message = result.err().unwrap_or_default();
                    self.revision += 1;
                    self.save()?;
                }
            }
        }
        let id = view.hwnd().0 as isize;
        if self.last_seen.get(&id) != Some(&self.revision)
            && view
                .post(&json!({"v":1,"event":"project-updated","data":{"message":self.message}}))
                .is_ok()
        {
            self.last_seen.insert(id, self.revision);
            let _ = view.post(&json!({"v":1,"event":"actions-changed","data":{}}));
        }
        Ok(())
    }
    fn event(&mut self, view: &View, event: &Value) -> Result<()> {
        if event["event"] == "files_dropped"
            && let Some(paths) = event["paths"].as_array()
        {
            for path in paths.iter().filter_map(Value::as_str) {
                let outcome = FileGrant::from_path(path).and_then(|grant| {
                    if grant.path().is_dir() {
                        self.open_folder(grant)
                    } else {
                        self.import_file(view, grant).map(|_| ())
                    }
                });
                if let Err(error) = outcome {
                    self.message = error;
                    self.revision += 1;
                }
            }
        }
        Ok(())
    }
}
impl Drop for Atlas {
    fn drop(&mut self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
        let _ = self.save();
    }
}
impl AppCommands for Atlas {
    fn workspace(&mut self, view: &View, _: Empty) -> Result<Workspace> {
        Ok(Workspace {
            role: view.label().into(),
            folder: self.saved.folder.as_ref().map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            }),
            selected: self.saved.selected.clone(),
            count: (self.bundled.len() + self.saved.images.len()) as u32,
            running: self.job.is_some(),
            skipped: self.skipped,
            generation: self.generation,
        })
    }
    fn page(&mut self, _: &View, args: PageRequest) -> Result<Vec<Asset>> {
        Ok(self
            .bundled
            .iter()
            .cloned()
            .chain(
                self.saved
                    .images
                    .iter()
                    .map(|item| item.metadata(self.saved.folder.as_deref())),
            )
            .skip(args.offset as usize)
            .take(args.limit.min(128) as usize)
            .collect())
    }
    fn thumbnails(&mut self, _: &View, args: Ids) -> Result<Vec<Thumbnail>> {
        Ok(args
            .ids
            .into_iter()
            .take(16)
            .filter(|id| self.positions.contains_key(id))
            .map(|id| {
                let url = self.thumbs.get(&id).cloned().or_else(|| {
                    std::fs::read_to_string(self.profile.join("thumbnails").join(&id)).ok()
                });
                Thumbnail { id, url }
            })
            .collect())
    }
    fn catalog(&mut self, view: &View, _: Empty) -> Result<Vec<Asset>> {
        self.page(
            view,
            PageRequest {
                offset: 0,
                limit: 128,
            },
        )
    }
    fn select_asset(&mut self, view: &View, args: Selection) -> Result<Asset> {
        let asset = self.paint(view, &args.id, 0)?;
        self.restore_after_scan = None;
        if self.saved.selected != args.id {
            self.saved.selected = args.id;
            self.dirty = true;
            self.revision += 1;
        }
        if self.job.is_none() {
            self.save()?;
        }
        Ok(asset)
    }
    fn render_asset(&mut self, view: &View, args: RenderRequest) -> Result<Asset> {
        self.paint(view, &args.id, args.viewport)
    }
    fn import_image(&mut self, view: &View, _: Empty) -> Result<Option<Asset>> {
        let Some(grant) = files::pick_file(view.hwnd(), "Import an image")? else {
            return Ok(None);
        };
        self.import_file(view, grant).map(Some)
    }
}
fn arg(flag: &str) -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        if a == flag {
            return args.next().map(Into::into);
        }
    }
    None
}
fn run() -> Result<()> {
    rustybox_sdk::enable_per_monitor_dpi();
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let assets = arg("--assets").unwrap_or_else(|| exe.with_file_name("atlas-ui"));
    let profile = arg("--profile").unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("RustyBoxUI/Atlas")
    });
    let hidden = std::env::args_os().any(|arg| arg == "--hidden");
    let windows = Windows::default();
    let mut state = Atlas::new(assets, profile, windows.clone(), hidden)?;
    if let Some(folder) = arg("--folder") {
        state.open_folder(FileGrant::from_path(folder)?)?;
    }
    windows.open("main", state.window("Atlas — Texture workspace")?);
    let mut app = App::new(state, register::<Atlas>(), windows);
    app.action(Action {
        id: "open-folder",
        title: "Open folder",
        shortcut: "Ctrl+Shift+O",
        enabled: |_, v| v.label() == "main",
        run: |s, v| {
            if let Some(folder) = files::pick_folder(v.hwnd(), "Open image folder")? {
                s.open_folder(folder)?;
            }
            Ok(())
        },
    });
    app.action(Action {
        id: "cancel-import",
        title: "Cancel import",
        shortcut: "Escape",
        enabled: |s, v| s.job.is_some() && v.label() == "main",
        run: |s, _| {
            if let Some(job) = &s.job {
                job.cancel();
            }
            Ok(())
        },
    });
    app.action(Action {
        id: "detach",
        title: "Detach preview",
        shortcut: "Ctrl+Shift+D",
        enabled: |_, v| v.label() == "main",
        run: |s, _| {
            s.windows
                .open("preview", s.window("Atlas — Detached preview")?);
            Ok(())
        },
    });
    app.on_event(Atlas::event);
    app.on_idle(Atlas::update);
    app.run()
}
fn main() {
    if let Err(error) = run() {
        if std::env::args_os().any(|v| v == "--hidden") {
            eprintln!("{error}");
        } else {
            rustybox_sdk::show_error(&error);
        }
        std::process::exit(1);
    }
}
