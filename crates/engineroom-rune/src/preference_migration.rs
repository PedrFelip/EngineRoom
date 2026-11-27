//! Copies preferences from the Rune preview into the final data directory.

use std::path::Path;

use crate::preferences;

/// Migrates the preview preferences if the destination does not already exist.
pub(crate) fn migrate_first_launch(base: &Path) -> Result<(), String> {
    let dirs =
        directories::BaseDirs::new().ok_or_else(|| "Diretório de dados indisponível".to_owned())?;
    let source = dirs
        .data_dir()
        .join("engineroom-experimental/preferences.json");
    migrate_paths(&base.join("preferences.json"), &source)
}

fn migrate_paths(destination: &Path, source: &Path) -> Result<(), String> {
    if destination.exists() {
        return Ok(());
    }
    if let Some(preferences) = preferences::read(source)? {
        preferences::write_if_missing(destination, &preferences)
            .map_err(|error| format!("Salvar preferências migradas: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_is_preserved_and_destination_takes_precedence() {
        let dir = tempfile::tempdir().expect("create temporary directory");
        let source = dir.path().join("preview.json");
        let destination = dir.path().join("final/preferences.json");
        std::fs::write(
            &source,
            include_str!("../tests/fixtures/rune-preview-preferences.json"),
        )
        .expect("write preview fixture");
        migrate_paths(&destination, &source).expect("migrate preview");
        let preview = preferences::read(&source).expect("read preview").unwrap();
        assert_eq!(preferences::read(&destination).unwrap(), Some(preview));
        let original = std::fs::read(&destination).expect("read destination");
        std::fs::write(&source, b"invalid").expect("alter preview");
        migrate_paths(&destination, &source).expect("keep destination");
        assert_eq!(std::fs::read(&destination).unwrap(), original);
    }

    #[test]
    fn invalid_preview_does_not_create_destination() {
        let dir = tempfile::tempdir().expect("create temporary directory");
        let source = dir.path().join("preview.json");
        let destination = dir.path().join("preferences.json");
        std::fs::write(&source, b"invalid").expect("write invalid preview");
        assert!(migrate_paths(&destination, &source).is_err());
        assert!(!destination.exists());
    }
}
