//! Candidate discovery never changes a Project. The selected folder bounds the
//! search; incomplete enumeration cannot prove that a match is unique.
use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

use myalbuns_paths::{ExpectedObject, RootBindingPlan};

use super::{MediaBinding, MediaResolver};

impl MediaResolver {
    pub(crate) fn find_relink_candidates(
        &self,
        folder: &Path,
        bindings: &[MediaBinding],
        roots: &RootBindingPlan,
    ) -> Result<HashMap<String, PathBuf>, String> {
        let mut matches: HashMap<OsString, Vec<PathBuf>> = bindings
            .iter()
            .filter_map(|binding| binding.logical_path.file_name())
            .map(|name| (name.to_owned(), Vec::new()))
            .collect();
        let mut pending = vec![folder.to_path_buf()];
        while let Some(path) = pending.pop() {
            let directory = roots
                .resolve_existing(&path, ExpectedObject::Directory)
                .map_err(|error| format!("Não foi possível verificar toda a pasta: {error}"))?;
            let entries = std::fs::read_dir(directory.operational_path())
                .map_err(|error| format!("Não foi possível verificar toda a pasta: {error}"))?;
            for entry in entries {
                let entry = entry.map_err(|error| error.to_string())?;
                let metadata =
                    std::fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
                // Do not follow junctions/symlinks outside the selected tree or
                // silently call a partly searched tree unique.
                if is_link(&metadata) {
                    return Err("A pasta contém um atalho ou junção. Escolha uma pasta de Fotos sem esses redirecionamentos.".into());
                }
                let child = path.join(entry.file_name());
                if metadata.is_dir() {
                    pending.push(child);
                } else if metadata.is_file()
                    && let Some(found) = matches.get_mut(&entry.file_name())
                {
                    found.push(child);
                }
            }
        }
        Ok(bindings
            .iter()
            .filter_map(|binding| {
                let found = matches.get(binding.logical_path.file_name()?)?;
                (found.len() == 1).then(|| (binding.media_id.clone(), found[0].clone()))
            })
            .collect())
    }
}

fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myalbuns_core::MediaKind;
    use myalbuns_paths::OperationPathContext;

    #[test]
    fn recursive_search_requires_one_exact_filename_and_retains_logical_paths() {
        let root = tempfile::tempdir().unwrap();
        for folder in ["Fotos/a", "Fotos/b"] {
            std::fs::create_dir_all(root.path().join(folder)).unwrap();
        }
        for name in [
            "Fotos/a/única.JPG",
            "Fotos/a/dupla.png",
            "Fotos/b/dupla.png",
            "Fotos/a/mesmo.jpg",
            "Fotos/b/mesmo.jpeg",
        ] {
            std::fs::write(root.path().join(name), b"candidate").unwrap();
        }
        let bindings = [
            "única.JPG",
            "dupla.png",
            "ausente.jpg",
            "mesmo.jpg",
            "Única.JPG",
        ]
        .map(|name| MediaBinding {
            media_id: name.into(),
            kind: MediaKind::Photo,
            logical_path: PathBuf::from(r"Z:\antiga").join(name),
        });
        let mut context = OperationPathContext::new();
        let folder = PathBuf::from(r"Z:\Fotos");
        context.capture_with_binding(&folder, root.path()).unwrap();
        let found = MediaResolver
            .find_relink_candidates(&folder, &bindings, &context.freeze())
            .unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found["única.JPG"], folder.join("a").join("única.JPG"));
        assert_eq!(found["mesmo.jpg"], folder.join("a").join("mesmo.jpg"));
    }
}
