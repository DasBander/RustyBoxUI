//! Cargo subcommand for creating, developing and distributing RustyBox applications.
use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};
type Result<T> = std::result::Result<T, String>;
const HELP: &str = "cargo rustybox new <directory> [--sdk <SDK>]\ncargo rustybox dev\ncargo rustybox build\ncargo rustybox package\ncargo rustybox publish\ncargo rustybox rollback <published-version>\nRun dev, build and package inside a generated project.";
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
        "new" if args.len() == 2 || (args.len() == 4 && args[2] == "--sdk") => {
            let root = sdk_root(args.get(3))?;
            ps(
                &root.join("scripts/new-app.ps1"),
                &["-Destination".into(), args[1].clone()],
            )
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
        "seal" if args.len() >= 4 => {
            let output = Path::new(&args[2]);
            if output.exists() {
                return Err("Seal into a fresh staging directory".into());
            }
            let name = output
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or("Invalid package filename")?;
            for exe in &args[3..] {
                if !Path::new(exe).is_file() {
                    return Err("Missing staged executable".into());
                }
            }
            let key = rustybox_assets::cook(Path::new(&args[1]), output)?;
            for exe in &args[3..] {
                rustybox_assets::bind(Path::new(exe), name, &key)?;
            }
            Ok(())
        }
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
