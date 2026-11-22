//! Cargo subcommand for creating, developing and distributing RustyBox applications.
mod doctor;
use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};
type Result<T> = std::result::Result<T, String>;
const HELP: &str = "cargo rustybox new <directory> [--template html|vite-ts] [--sdk <SDK>]\ncargo rustybox dev\ncargo rustybox build\ncargo rustybox package\ncargo rustybox doctor [directory|host.exe]\ncargo rustybox content <keygen|pack> ...\ncargo rustybox publish\ncargo rustybox rollback <published-version>\nRun dev, build and package inside a generated project.";
fn exec(program: &str, args: &[OsString]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|e| format!("{program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}"))
    }
}
fn ps(script: &Path, args: &[OsString]) -> Result<()> {
    if !script.is_file() {
        return Err(format!(
            "Missing {}. Run this command in a generated project.",
            script.display()
        ));
    }
    let mut command: Vec<OsString> = ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]
        .into_iter()
        .map(Into::into)
        .collect();
    command.push(script.as_os_str().into());
    command.extend_from_slice(args);
    exec("powershell.exe", &command)
}
fn sdk_root(explicit: Option<&OsString>) -> Result<PathBuf> {
    let mut paths = Vec::new();
    if let Some(root) = explicit {
        paths.push(PathBuf::from(root));
    } else if let Some(root) = env::var_os("RUSTYBOX_SDK") {
        paths.push(root.into());
    } else {
        for path in [
            env::current_dir().map_err(|e| e.to_string())?,
            env::current_exe().map_err(|e| e.to_string())?,
        ] {
            paths.extend(path.ancestors().map(Path::to_path_buf));
        }
    }
    paths
        .into_iter()
        .find(|p| p.join("scripts/new-app.ps1").is_file())
        .ok_or("Set RUSTYBOX_SDK or pass --sdk pointing to the SDK or built checkout".into())
}
/// Bind the staged hosts only after the index and every requested part have been written.
fn seal(mut args: &[OsString]) -> Result<()> {
    let mut options = rustybox_assets::PackOptions::default();
    if args.first().is_some_and(|arg| arg == "--manifest") {
        let path = args
            .get(1)
            .ok_or("seal --manifest needs a project manifest")?;
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read project manifest: {e}"))?;
        let manifest: serde_json::Value =
            serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
        if let Some(settings) = manifest.get("resources") {
            options = serde_json::from_value(settings.clone())
                .map_err(|e| format!("Invalid resources settings: {e}"))?;
        }
        args = &args[2..];
    }
    let [root, output, hosts @ ..] = args else {
        return Err("Usage: seal [--manifest rustybox.json] <web-root> <package> <exe>...".into());
    };
    let output = Path::new(output);
    if output.exists() {
        return Err("Seal into a fresh staging directory".into());
    }
    let name = output
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("Invalid package filename")?;
    if hosts.is_empty() || hosts.iter().any(|exe| !Path::new(exe).is_file()) {
        return Err("Missing staged executable".into());
    }
    let key = rustybox_assets::cook_with_options(Path::new(root), output, options)?;
    for exe in hosts {
        rustybox_assets::bind(Path::new(exe), name, &key)?;
    }
    Ok(())
}

fn run() -> Result<()> {
    let mut args: Vec<_> = env::args_os().skip(1).collect();
    if args.first().is_some_and(|v| v == "rustybox") {
        args.remove(0);
    }
    let cwd = env::current_dir().map_err(|e| e.to_string())?;
    let command = args.first().and_then(|v| v.to_str()).unwrap_or("--help");
    match command {
        "--help" | "-h" => {
            println!("{HELP}");
            Ok(())
        }
        "new" if args.len() >= 2 => {
            let mut sdk = None;
            let mut options = vec!["-Destination".into(), args[1].clone()];
            for pair in args[2..].chunks(2) {
                match pair {
                    [flag, value] if flag == "--sdk" => sdk = Some(value),
                    [flag, value] if flag == "--template" => {
                        options.extend(["-Template".into(), value.clone()]);
                    }
                    _ => return Err(HELP.into()),
                }
            }
            ps(&sdk_root(sdk)?.join("scripts/new-app.ps1"), &options)
        }
        "doctor" if args.len() <= 2 => doctor::run(args.get(1).map(Path::new)),
        "content" => {
            let local = cwd.join("tools/content-packs.mjs");
            let script = if local.is_file() {
                local
            } else {
                sdk_root(None)?.join("scripts/content-packs.mjs")
            };
            let mut options = vec![script.into_os_string()];
            options.extend_from_slice(&args[1..]);
            exec("node", &options)
        }
        "dev" if args.len() == 1 => exec("node", &[cwd.join("tools/dev.mjs").into_os_string()]),
        "build" if args.len() == 1 => ps(&cwd.join("build.ps1"), &[]),
        "package" if args.len() == 1 => ps(&cwd.join("tools/package-app.ps1"), &[]),
        "publish" if args.len() == 1 => ps(&cwd.join("tools/publish.ps1"), &[]),
        "rollback" if args.len() == 2 => ps(
            &cwd.join("tools/publish.ps1"),
            &["-Rollback".into(), args[1].clone()],
        ),
        "exe-name" if args.len() == 2 => {
            let info =
                rustybox_build::Metadata::read(Path::new(&args[1])).map_err(|e| e.to_string())?;
            println!("{}", info.executable_name("rustybox-starter"));
            Ok(())
        }
        "msix-icons" if args.len() == 3 => {
            rustybox_build::write_msix_icons(Path::new(&args[1]), Path::new(&args[2]))
                .map_err(|e| e.to_string())
        }
        "seal" => seal(&args[1..]),
        _ => Err(HELP.into()),
    }
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
