//! Build-time integration for native Windows application branding.
mod icon;
mod metadata;
use icon::Artwork;
pub use metadata::Metadata;
use std::{env, fs, path::Path};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Call from build.rs. The target is the Cargo binary's name, before any distribution rename.
pub fn embed(binary: &str) -> Result<()> {
    let manifest = Path::new("rustybox.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let info = Metadata::read(manifest)?;
    println!(
        "cargo:rustc-env=RUSTYBOX_APP_ID={}",
        info.identity.as_deref().unwrap_or(binary)
    );
    println!(
        "cargo:rustc-env=RUSTYBOX_FILE_TYPES={}",
        info.file_types.join(",")
    );
    println!(
        "cargo:rustc-env=RUSTYBOX_PROTOCOLS={}",
        info.protocols.join(",")
    );
    if let Some(icon) = &info.icon {
        println!("cargo:rerun-if-changed={}", icon.display());
    }
    let out = std::path::PathBuf::from(env::var_os("OUT_DIR").ok_or("Cargo OUT_DIR is missing")?);
    let icon = out.join("rustybox-app.ico");
    Artwork::load(info.icon.as_deref())?.write_ico(&icon)?;
    let rc = out.join("rustybox-app.rc");
    let script = resource_script(&info, binary, &icon)?;
    // rc.exe reads UTF-16LE with a BOM independently of the build machine's active code page.
    let utf16: Vec<u8> = std::iter::once(0xfeff)
        .chain(script.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    fs::write(&rc, utf16)?;
    embed_resource::compile_for(&rc, [binary], embed_resource::NONE)
        .manifest_required()
        .map_err(|e| format!("Windows resource compilation failed: {e:?}"))?;
    Ok(())
}

/// Write the logo files referenced by an MSIX manifest.
pub fn write_msix_icons(manifest: &Path, directory: &Path) -> Result<()> {
    let info = Metadata::read(manifest)?;
    Artwork::load(info.icon.as_deref())?.write_tiles(directory)
}

fn rc_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\"\"")
}

fn resource_script(info: &Metadata, binary: &str, icon: &Path) -> Result<String> {
    let original = info.executable_name(binary);
    let file_version = info.file_version.as_deref().unwrap_or(&info.version);
    let fields = [
        (
            "CompanyName",
            Some(
                info.company_name
                    .as_deref()
                    .unwrap_or(&info.publisher_display_name),
            ),
        ),
        (
            "FileDescription",
            Some(info.description.as_deref().unwrap_or(&info.name)),
        ),
        ("FileVersion", Some(file_version)),
        ("InternalName", Some(binary)),
        ("OriginalFilename", Some(original.as_str())),
        ("ProductName", Some(info.name.as_str())),
        ("ProductVersion", Some(info.version.as_str())),
        ("LegalCopyright", info.copyright.as_deref()),
        ("LegalTrademarks", info.trademarks.as_deref()),
    ];
    let strings = fields
        .into_iter()
        .filter_map(|(key, value)| {
            value.map(|s| format!("      VALUE \"{key}\", L\"{}\"", rc_string(s)))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let numbers = |field, value| -> Result<String> {
        let parts = metadata::version_parts(field, value)?;
        Ok(parts.map(|part| part.to_string()).join(","))
    };
    Ok(format!(
        include_str!("app.rc"),
        icon = rc_string(&icon.to_string_lossy()),
        file = numbers("fileVersion", file_version)?,
        product = numbers("version", &info.version)?,
        strings = strings
    ))
}
