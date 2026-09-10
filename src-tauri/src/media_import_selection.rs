//! File and folder selections converge before the single native import attempt.
use crate::ipc_contract::ImageProcessingProblem;
use myalbuns_paths::{ExpectedObject, OperationPathContext};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

pub(crate) fn expand(selected: Vec<PathBuf>) -> (Vec<PathBuf>, Vec<ImageProcessingProblem>) {
    let mut context = OperationPathContext::new();
    let mut problems = Vec::new();
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for path in selected {
        if !seen.insert(path.clone()) {
            continue;
        }
        let _ = context.capture(&path);
        candidates.push(path);
    }
    let plan = context.freeze();
    let mut files = Vec::new();
    for path in candidates {
        match plan.resolve_existing(&path, ExpectedObject::Directory) {
            Ok(directory) => match std::fs::read_dir(directory.operational_path()) {
                Ok(entries) => {
                    let mut children = Vec::new();
                    for entry in entries {
                        match entry {
                            Ok(entry) => match entry.file_type() {
                                Ok(kind)
                                    if !kind.is_dir() && supported_extension(&entry.path()) =>
                                {
                                    children.push(path.join(entry.file_name()))
                                }
                                Ok(_) => {}
                                Err(error) => problems.push(problem(
                                    &path.join(entry.file_name()),
                                    error.to_string(),
                                )),
                            },
                            Err(error) => problems.push(problem(&path, error.to_string())),
                        }
                    }
                    children.sort();
                    files.extend(children);
                }
                Err(error) => problems.push(problem(&path, error.to_string())),
            },
            // Files, including missing or invalid ones, reach the shared inspector;
            // it can preserve an existing link without rereading its Original.
            Err(_) => files.push(path),
        }
    }
    let mut seen = HashSet::new();
    files.retain(|path| seen.insert(path.clone()));
    (files, problems)
}

fn supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["jpg", "jpeg", "png", "tif", "tiff"]
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

fn problem(path: &Path, reason: String) -> ImageProcessingProblem {
    ImageProcessingProblem {
        file_name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folder_and_drop_keep_direct_images_once_and_do_not_walk_subfolders() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("Fotos");
        std::fs::create_dir_all(folder.join("subpasta")).unwrap();
        for name in [
            "a.JPG",
            "b.png",
            "c.tiff",
            "leia.txt",
            "subpasta/oculta.jpg",
        ] {
            std::fs::write(folder.join(name), b"selected candidate").unwrap();
        }
        let explicit = root.path().join("explicit.bmp");
        std::fs::write(&explicit, b"selected unsupported image").unwrap();
        let (files, problems) = expand(vec![
            folder.clone(),
            folder.join("a.JPG"),
            folder.clone(),
            explicit.clone(),
        ]);
        assert!(problems.is_empty());
        assert_eq!(
            files,
            vec![
                folder.join("a.JPG"),
                folder.join("b.png"),
                folder.join("c.tiff"),
                explicit
            ]
        );
        let absent = root.path().join("previously imported.jpg");
        assert_eq!(
            expand(vec![absent.clone()]).0,
            vec![absent],
            "reselection is decided by the import catalog even when the Original is gone"
        );
    }
}
