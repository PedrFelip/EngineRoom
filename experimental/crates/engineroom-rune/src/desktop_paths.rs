//! Paths owned by the experimental desktop application.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

/// Returns the experimental application's data directory without creating it.
///
/// # Errors
/// Returns an error if the operating system cannot provide a data directory.
pub fn data_dir() -> io::Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|dirs| dirs.data_dir().join("engineroom-experimental"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "data directory unavailable"))
}

/// Resolves Stockfish from the override, installed resources, or development resources.
///
/// A sidecar beside the executable takes precedence over the override. Development
/// builds fall back to `resources/stockfish` and then to the main app's bundled
/// Tauri sidecar under `app/src-tauri/binaries`. This function does not launch
/// the executable.
///
/// # Errors
/// Returns filesystem errors, including an inaccessible or missing executable.
pub fn stockfish_path() -> io::Result<PathBuf> {
    let executable = env::current_exe()?;
    resolve_stockfish(
        env::var_os("ENGINE_ROOM_STOCKFISH")
            .map(PathBuf::from)
            .as_deref(),
        &executable,
        &workspace_root(),
        cfg!(debug_assertions),
        cfg!(windows),
    )
}

/// Returns the experimental workspace root from this crate's manifest directory.
pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn resolve_stockfish(
    override_path: Option<&Path>,
    executable: &Path,
    manifest: &Path,
    development: bool,
    windows: bool,
) -> io::Result<PathBuf> {
    let name = if windows {
        "stockfish.exe"
    } else {
        "stockfish"
    };
    let parent = executable.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "executable has no parent directory",
        )
    })?;
    match validate(&parent.join(name)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        result => return result,
    }
    if let Some(path) = override_path {
        return validate(path);
    }
    let installed = parent.join("resources").join(name);
    match validate(&installed) {
        Err(error) if error.kind() == io::ErrorKind::NotFound && development => {
            match validate(&manifest.join("resources").join(name)) {
                Err(fallback) if fallback.kind() == io::ErrorKind::NotFound => {
                    dev_sidecar(manifest, windows)
                }
                result => result,
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Stockfish ausente; reinstale o aplicativo ou configure ENGINE_ROOM_STOCKFISH",
        )),
        result => result,
    }
}

/// Finds the main app's downloaded Tauri sidecar during development.
///
/// The sidecar is named `stockfish-<target-triple>` (with `.exe` on Windows).
/// Resolution scans deterministic names instead of guessing a triple, so a
/// locally downloaded binary keeps working regardless of the host triple.
fn dev_sidecar(manifest: &Path, windows: bool) -> io::Result<PathBuf> {
    let directory = manifest.join("../app/src-tauri/binaries");
    let missing = |directory: &Path| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "no Stockfish in {}; set ENGINE_ROOM_STOCKFISH or install resources/stockfish",
                directory.display()
            ),
        )
    };
    let mut candidates: Vec<PathBuf> = fs::read_dir(&directory)
        .map_err(|_| missing(&directory))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with("stockfish-") && name.ends_with(".exe") == windows
                    })
        })
        .collect();
    candidates.sort();
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| missing(&directory))
}

fn validate(path: &Path) -> io::Result<PathBuf> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a file", path.display()),
        ));
    }
    Ok(path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_respect_override_install_and_dev_fallback() {
        let root = env::temp_dir().join(format!("engineroom-paths-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("install/resources")).expect("create install resources");
        fs::create_dir_all(root.join("dev/resources")).expect("create dev resources");
        let exe = root.join("install/app");
        let dev = root.join("dev");
        let installed = root.join("install/resources/stockfish.exe");
        let fallback = dev.join("resources/stockfish.exe");
        fs::write(&fallback, "dev").expect("write fallback");
        assert_eq!(
            resolve_stockfish(None, &exe, &dev, true, true).expect("dev fallback"),
            fallback
        );
        assert_eq!(
            resolve_stockfish(None, &exe, &dev, false, true)
                .expect_err("release must not use fallback")
                .kind(),
            io::ErrorKind::NotFound
        );
        fs::write(&installed, "installed").expect("write installed");
        assert_eq!(
            resolve_stockfish(None, &exe, &dev, true, true).expect("installed"),
            installed
        );
        assert_eq!(
            resolve_stockfish(Some(&fallback), &exe, &dev, true, true).expect("override"),
            fallback
        );
        assert_eq!(
            resolve_stockfish(Some(&root), &exe, &dev, true, true)
                .expect_err("directory override")
                .kind(),
            io::ErrorKind::InvalidInput
        );
        fs::remove_dir_all(root).expect("remove test files");
    }
    #[test]
    fn packaged_sidecar_precedes_override_and_missing_engine_is_reported() {
        let root =
            env::temp_dir().join(format!("engineroom-packaged-paths-{}", std::process::id()));
        fs::create_dir_all(&root).expect("create package directory");
        for windows in [false, true] {
            let exe = root.join("engineroom-rune");
            let error = resolve_stockfish(None, &exe, &root, false, windows)
                .expect_err("missing engine must be an error, not a panic");
            assert_eq!(error.kind(), io::ErrorKind::NotFound);
            assert!(error.to_string().contains("Stockfish ausente"));
            let sidecar = root.join(if windows {
                "stockfish.exe"
            } else {
                "stockfish"
            });
            fs::write(&sidecar, "engine").expect("write sidecar");
            assert_eq!(
                resolve_stockfish(Some(&root.join("missing")), &exe, &root, false, windows)
                    .expect("packaged engine"),
                sidecar
            );
            fs::remove_file(sidecar).expect("remove sidecar");
        }
        fs::remove_dir_all(root).expect("remove package directory");
    }

    #[test]
    fn development_falls_back_to_the_main_app_sidecar() {
        let root = env::temp_dir().join(format!("engineroom-paths-sidecar-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let binaries = root.join("app/src-tauri/binaries");
        fs::create_dir_all(&binaries).expect("create sidecar dir");
        let exe = root.join("install/app");
        let manifest = root.join("experimental");
        fs::create_dir_all(&manifest).expect("create experimental workspace");
        assert!(
            resolve_stockfish(None, &exe, &manifest, true, true).is_err(),
            "nothing to resolve before the sidecar exists"
        );
        let sidecar = binaries.join("stockfish-x86_64-pc-windows-msvc.exe");
        fs::write(&sidecar, "sidecar").expect("write sidecar");
        assert_eq!(
            resolve_stockfish(None, &exe, &manifest, true, true)
                .expect("sidecar")
                .canonicalize()
                .expect("canonical sidecar"),
            sidecar
        );
        assert!(
            resolve_stockfish(None, &exe, &manifest, false, true).is_err(),
            "release builds must not use development sidecars"
        );
        fs::remove_dir_all(root).expect("remove test files");
    }
}
