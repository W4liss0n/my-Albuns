//! Candidate discovery never changes a Project. The selected folder bounds the
//! search to its immediate files; incomplete enumeration cannot prove uniqueness.
use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

use myalbuns_paths::RootBindingPlan;

use super::{MediaBinding, MediaResolver};
use crate::linked_files::LinkedFiles;

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
        let listed = LinkedFiles::new()
            .list_folder(roots, folder)
            .map_err(folder_inspection_failure)?;
        for entry in listed.entries {
            let Some(found) = matches.get_mut(&entry.name) else {
                continue;
            };
            if entry.link {
                return Err("A imagem encontrada é um atalho ou redirecionamento. Escolha a pasta que contém o arquivo original.".into());
            }
            if !entry.directory {
                found.push(folder.join(&entry.name));
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

fn folder_inspection_failure(error: impl std::fmt::Display) -> String {
    crate::linked_files::inspection_failure(
        error,
        "Não foi possível verificar todos os arquivos da pasta. Confira se ela está disponível e se você tem permissão para acessá-la.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use myalbuns_core::MediaKind;
    use myalbuns_paths::OperationPathContext;

    #[test]
    fn candidate_inspection_keeps_the_search_binding_after_a_new_mapping_is_captured() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let folder = PathBuf::from(r"Z:\Fotos");
        let binding = MediaBinding {
            media_id: "photo-1".into(),
            kind: MediaKind::Photo,
            logical_path: PathBuf::from(r"Z:\antiga\Foto.png"),
        };
        for (root, width) in [(first.path(), 17), (second.path(), 41)] {
            std::fs::create_dir(root.join("Fotos")).unwrap();
            image::RgbImage::from_pixel(width, 11, image::Rgb([30, 80, 140]))
                .save_with_format(root.join("Fotos/Foto.png"), image::ImageFormat::Png)
                .unwrap();
        }
        let mut first_context = OperationPathContext::new();
        first_context
            .capture_with_binding(&folder, first.path())
            .unwrap();
        let first_plan = first_context.freeze();
        let candidates = MediaResolver
            .find_relink_candidates(&folder, std::slice::from_ref(&binding), &first_plan)
            .unwrap();
        let mut remapped_context = OperationPathContext::new();
        remapped_context
            .capture_with_binding(&folder, second.path())
            .unwrap();
        let replacement = candidates[&binding.media_id].clone();
        let inspected = MediaResolver
            .propose_relink_in_plan(&binding, replacement.clone(), &first_plan)
            .unwrap();
        let remapped = MediaResolver
            .propose_relink_in_plan(&binding, replacement, &remapped_context.freeze())
            .unwrap();
        assert_ne!(
            inspected.source_metadata(),
            remapped.source_metadata(),
            "the next attempt sees the remap; this attempt retains the source that it searched"
        );
    }

    #[test]
    fn selected_folder_search_ignores_subfolders_and_requires_exact_filename() {
        let root = tempfile::tempdir().unwrap();
        for folder in ["Fotos/a", "Fotos/b"] {
            std::fs::create_dir_all(root.path().join(folder)).unwrap();
        }
        for name in [
            "Fotos/única.JPG",
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
        assert_eq!(found.len(), 1, "subfolders must not contribute candidates");
        assert_eq!(found["única.JPG"], folder.join("única.JPG"));
    }

    #[test]
    fn relink_is_refused_while_the_original_cannot_be_proved_absent() {
        let root = tempfile::tempdir().unwrap();
        let replacement = root.path().join("renamed.png");
        image::RgbImage::new(16, 24).save(&replacement).unwrap();
        // A folder where the Original was answers, but not as a missing file.
        let occupied = root.path().join("occupied.png");
        std::fs::create_dir(&occupied).unwrap();
        let mut context = OperationPathContext::new();
        context.capture(&replacement).unwrap();
        let roots = context.freeze();
        let propose = |logical_path: PathBuf| {
            MediaResolver.propose_relink_in_plan(
                &MediaBinding {
                    media_id: "selected".into(),
                    kind: MediaKind::Photo,
                    logical_path,
                },
                replacement.clone(),
                &roots,
            )
        };

        assert!(propose(root.path().join("absent.png")).is_ok());
        assert!(propose(occupied).is_err());
        // A drive that is no longer mapped has no root in the attempt's plan.
        let unmapped = PathBuf::from(r"\\servidor\acervo\Fotos\original.png");
        assert!(!roots.covers(&unmapped));
        assert!(propose(unmapped).is_err());
    }

    /// A junction redirects a folder without the privilege a symbolic link needs.
    #[cfg(windows)]
    fn create_junction(link: &Path, target: &Path) {
        let output = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_found_entry_that_is_a_link_is_refused_instead_of_followed() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("Fotos");
        let elsewhere = root.path().join("Outro local");
        for directory in [&folder, &elsewhere] {
            std::fs::create_dir(directory).unwrap();
        }
        std::fs::write(folder.join("direta.png"), b"candidate").unwrap();
        let binding = |name: &str| MediaBinding {
            media_id: name.into(),
            kind: MediaKind::Photo,
            logical_path: PathBuf::from(r"Z:\antiga").join(name),
        };
        let mut context = OperationPathContext::new();
        context.capture(&folder).unwrap();
        let roots = context.freeze();
        let find = |bindings: &[MediaBinding]| {
            MediaResolver.find_relink_candidates(&folder, bindings, &roots)
        };

        create_junction(&folder.join("redirecionada.png"), &elsewhere);
        let direct = find(&[binding("direta.png")]).unwrap();
        assert_eq!(
            direct["direta.png"],
            folder.join("direta.png"),
            "a link nobody asked for does not disturb the search"
        );
        let refused = find(&[binding("direta.png"), binding("redirecionada.png")]).unwrap_err();

        std::fs::remove_dir(&elsewhere).unwrap();
        assert_eq!(
            find(&[binding("redirecionada.png")]).unwrap_err(),
            refused,
            "a broken link is refused the same way"
        );
        let unlisted = MediaResolver
            .find_relink_candidates(&elsewhere, &[binding("direta.png")], &roots)
            .unwrap_err();
        assert_ne!(
            refused, unlisted,
            "the refusal blames the link, not the folder"
        );
    }

    #[test]
    fn a_folder_that_cannot_be_listed_is_reported_instead_of_searched() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("Fotos.png");
        std::fs::write(&file, b"not a folder").unwrap();
        let binding = MediaBinding {
            media_id: "photo-1".into(),
            kind: MediaKind::Photo,
            logical_path: PathBuf::from(r"Z:\antiga\Foto.png"),
        };
        let mut context = OperationPathContext::new();
        context.capture(root.path()).unwrap();
        let roots = context.freeze();
        let find = |folder: &Path| {
            MediaResolver.find_relink_candidates(folder, std::slice::from_ref(&binding), &roots)
        };

        assert_eq!(find(root.path()), Ok(HashMap::new()));
        let missing = find(&root.path().join("Removida")).unwrap_err();
        assert_eq!(find(&file).unwrap_err(), missing);
        // A share that is not bound in the attempt's plan cannot be reached.
        let unreachable = PathBuf::from(r"\\servidor\acervo\Fotos");
        assert!(!roots.covers(&unreachable));
        assert_eq!(find(&unreachable).unwrap_err(), missing);
    }

    #[test]
    fn replacement_accepts_present_or_absent_images_but_relink_still_requires_absence() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original.png");
        let replacement = root.path().join("renamed.png");
        let corrupt = root.path().join("corrupt.png");
        image::RgbImage::new(24, 16).save(&original).unwrap();
        image::RgbImage::new(16, 24).save(&replacement).unwrap();
        std::fs::write(&corrupt, b"not an image").unwrap();
        let mut context = OperationPathContext::new();
        context.capture(&original).unwrap();
        let roots = context.freeze();
        for kind in [MediaKind::Photo, MediaKind::Decorative] {
            for name in ["original.png", "absent.png"] {
                let binding = MediaBinding {
                    media_id: "selected".into(),
                    kind,
                    logical_path: root.path().join(name),
                };
                let proposal = MediaResolver
                    .propose_replacement_in_plan(&binding, replacement.clone(), &roots)
                    .unwrap();
                assert_eq!(proposal.media_id(), "selected");
                assert_eq!(proposal.kind(), kind);
                assert_eq!(proposal.expected_logical_path(), binding.logical_path);
                assert_eq!(proposal.replacement_path(), replacement);
                assert!(
                    MediaResolver
                        .propose_replacement_in_plan(&binding, corrupt.clone(), &roots)
                        .is_err()
                );
                if name == "original.png" {
                    assert!(
                        MediaResolver
                            .propose_relink_in_plan(&binding, replacement.clone(), &roots)
                            .is_err()
                    );
                }
            }
        }
    }
}
