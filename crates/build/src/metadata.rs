//! Windows file properties and installer branding come from the same project manifest.
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub identity: Option<String>,
    #[serde(default)]
    pub file_types: Vec<String>,
    #[serde(default)]
    pub protocols: Vec<String>,
    pub name: String,
    pub version: String,
    pub publisher_display_name: String,
    pub file_version: Option<String>,
    pub executable_name: Option<String>,
    pub description: Option<String>,
    pub company_name: Option<String>,
    pub copyright: Option<String>,
    pub trademarks: Option<String>,
    pub icon: Option<PathBuf>,
}

impl Metadata {
    pub fn read(path: &Path) -> crate::Result<Self> {
        let json = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut info: Self = serde_json::from_str(json.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if let Some(identity) = &info.identity
            && (!(3..=50).contains(&identity.len())
                || !identity
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-')))
        {
            return Err("identity must be 3 to 50 letters, digits, periods or hyphens".into());
        }
        for ext in &info.file_types {
            let valid = ext.strip_prefix('.').is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix.len() <= 32
                    && suffix
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            });
            if !valid {
                return Err(
                    format!("Invalid fileTypes extension: {ext} (example: .rbnote)").into(),
                );
            }
        }
        for scheme in &info.protocols {
            if scheme.len() > 39
                || !scheme.starts_with(|c: char| c.is_ascii_lowercase())
                || !scheme.bytes().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'+' | b'.' | b'-')
                })
            {
                return Err(
                    format!("Invalid protocols scheme: {scheme} (example: rustybox-note)").into(),
                );
            }
        }
        for values in [&info.file_types, &info.protocols] {
            if values
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != values.len()
            {
                return Err("Each file extension or protocol may only be registered once".into());
            }
        }
        version_parts("version", &info.version)?;
        if let Some(version) = &info.file_version {
            version_parts("fileVersion", version)?;
        }
        for (field, text) in [
            ("name", Some(&info.name)),
            ("publisherDisplayName", Some(&info.publisher_display_name)),
            ("description", info.description.as_ref()),
            ("companyName", info.company_name.as_ref()),
            ("copyright", info.copyright.as_ref()),
            ("trademarks", info.trademarks.as_ref()),
        ] {
            if let Some(text) = text
                && (text.trim().is_empty() || text.chars().any(char::is_control))
            {
                return Err(format!("{field} must be a nonempty, single-line string").into());
            }
        }
        if let Some(name) = &info.executable_name {
            check_filename(name)?;
        }
        if let Some(icon) = &mut info.icon {
            *icon = path.parent().unwrap_or(Path::new(".")).join(&*icon);
        }
        Ok(info)
    }

    pub fn executable_name(&self, binary: &str) -> String {
        self.executable_name
            .clone()
            .unwrap_or_else(|| format!("{binary}.exe"))
    }
}

pub(crate) fn version_parts(field: &str, version: &str) -> crate::Result<[u16; 4]> {
    let invalid = || {
        format!("{field} must have four decimal components from 0 to 65535 (for example 1.2.3.0)")
    };
    let mut parts = [0u16; 4];
    let mut values = version.split('.');
    for part in &mut parts {
        let text = values.next().ok_or_else(invalid)?;
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid().into());
        }
        *part = text.parse().map_err(|_| invalid())?;
    }
    if values.next().is_some() {
        return Err(invalid().into());
    }
    Ok(parts)
}

fn check_filename(name: &str) -> crate::Result<()> {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let device = stem
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_ascii_uppercase();
    let numbered = device
        .strip_prefix("COM")
        .or_else(|| device.strip_prefix("LPT"));
    let reserved = matches!(
        device.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || numbered.is_some_and(|n| {
        matches!(
            n,
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        )
    });
    if name.encode_utf16().count() > 255
        || !name.to_ascii_lowercase().ends_with(".exe")
        || stem.trim_matches(['.', ' ']).is_empty()
        || name.starts_with(['.', ' '])
        || reserved
        || name
            .chars()
            .any(|c| c.is_control() || r#"<>:"/\|?*"#.contains(c))
    {
        return Err("executableName must be a single valid Windows filename ending in .exe".into());
    }
    Ok(())
}
