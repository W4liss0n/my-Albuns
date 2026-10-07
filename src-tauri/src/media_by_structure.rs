//! Finds Arquivos vinculados again when a Project folder was copied or moved
//! with the folders around it (ADR 0016). The copy is looked for first, so a
//! server that no longer answers is never waited for.

use std::path::{Path, PathBuf};

use myalbuns_core::MediaKind;
use myalbuns_paths::{RootBindingPlan, same_path, structural_candidates};

use crate::{
    linked_files::{LinkedFiles, MediaAvailability},
    media_runtime::MediaBinding,
};

/// A media whose file exists where the Project's folder structure says.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FoundMedia {
    pub(crate) media_id: String,
    pub(crate) stored: PathBuf,
    pub(crate) found: PathBuf,
}

/// For each binding, the first structural candidate that exists and can be
/// read, nearest folder first. A path another media of the same kind already
/// uses is left out, as an explicit Religação would refuse it.
///
/// `plan` must cover the Project's own root: every candidate lives under it.
pub(crate) fn find_media_by_structure(
    plan: &RootBindingPlan,
    project_path: &Path,
    bindings: &[MediaBinding],
) -> Vec<FoundMedia> {
    let Some(project_folder) = project_path.parent() else {
        return Vec::new();
    };
    let candidates = bindings
        .iter()
        .enumerate()
        .flat_map(|(index, binding)| {
            structural_candidates(project_folder, &binding.logical_path)
                .into_iter()
                .filter(|candidate| plan.covers(candidate))
                .map(move |candidate| {
                    (
                        index,
                        MediaBinding {
                            media_id: binding.media_id.clone(),
                            kind: binding.kind,
                            logical_path: candidate,
                        },
                    )
                })
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Vec::new();
    }
    let observations =
        LinkedFiles::new().observe(plan, candidates.iter().map(|(_, candidate)| candidate));
    let mut first_found: Vec<Option<PathBuf>> = vec![None; bindings.len()];
    for ((index, candidate), observation) in candidates.into_iter().zip(observations) {
        if first_found[index].is_none() && observation.availability == MediaAvailability::Candidate
        {
            first_found[index] = Some(candidate.logical_path);
        }
    }

    let mut in_use: Vec<(MediaKind, PathBuf)> = bindings
        .iter()
        .map(|binding| (binding.kind, binding.logical_path.clone()))
        .collect();
    let mut found_media = Vec::new();
    for (index, (binding, found)) in bindings.iter().zip(first_found).enumerate() {
        let Some(found) = found else {
            continue;
        };
        let taken = in_use.iter().enumerate().any(|(other, (kind, path))| {
            other != index && *kind == binding.kind && same_path(path, &found)
        });
        if taken {
            continue;
        }
        in_use[index].1 = found.clone();
        found_media.push(FoundMedia {
            media_id: binding.media_id.clone(),
            stored: binding.logical_path.clone(),
            found,
        });
    }
    found_media
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use myalbuns_core::MediaKind;
    use myalbuns_paths::OperationPathContext;

    use super::{FoundMedia, find_media_by_structure};
    use crate::media_runtime::MediaBinding;

    const UNREACHABLE: &str = r"\\myalbuns-unreachable.invalid\Servicos\Clientes";

    fn binding(id: &str, kind: MediaKind, path: impl Into<PathBuf>) -> MediaBinding {
        MediaBinding {
            media_id: id.into(),
            kind,
            logical_path: path.into(),
        }
    }

    fn write(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"image").unwrap();
    }

    fn found(
        project_path: &Path,
        bindings: &[MediaBinding],
    ) -> (Vec<FoundMedia>, std::time::Duration) {
        let mut paths = OperationPathContext::new();
        paths.capture(project_path).unwrap();
        let started = std::time::Instant::now();
        let found = find_media_by_structure(&paths.freeze(), project_path, bindings);
        (found, started.elapsed())
    }

    #[test]
    fn a_copied_job_folder_finds_its_photos_and_decoratives_without_asking_the_old_server() {
        let copy = tempfile::tempdir().unwrap();
        let job = copy.path().join("Job 040 - Escola Exemplo");
        let project_path = job.join("02 - Templates").join("Modelo.myalbuns");
        write(&project_path);
        write(&job.join(r"02 - Templates\MOLDURA.png"));
        write(&job.join(r"03 - Artes\Mensagem Final.png"));
        write(&job.join(r"02 - Templates\_qq\QQ\P00084_I.jpg"));
        let stored = Path::new(UNREACHABLE).join("Job 040 - Escola Exemplo");
        let bindings = [
            binding(
                "frame",
                MediaKind::Decorative,
                stored.join(r"02 - Templates\MOLDURA.png"),
            ),
            binding(
                "message",
                MediaKind::Decorative,
                stored.join(r"03 - Artes\Mensagem Final.png"),
            ),
            binding(
                "photo",
                MediaKind::Photo,
                stored.join(r"02 - Templates\_qq\QQ\P00084_I.jpg"),
            ),
            binding(
                "elsewhere",
                MediaKind::Decorative,
                Path::new(UNREACHABLE).join(r"Banco\Fundo.png"),
            ),
            binding(
                "not copied",
                MediaKind::Photo,
                stored.join(r"02 - Templates\_qq\QQ\P00085_I.jpg"),
            ),
        ];

        let (found, elapsed) = found(&project_path, &bindings);

        assert_eq!(
            found,
            [
                FoundMedia {
                    media_id: "frame".into(),
                    stored: bindings[0].logical_path.clone(),
                    found: job.join(r"02 - Templates\MOLDURA.png"),
                },
                FoundMedia {
                    media_id: "message".into(),
                    stored: bindings[1].logical_path.clone(),
                    found: job.join(r"03 - Artes\Mensagem Final.png"),
                },
                FoundMedia {
                    media_id: "photo".into(),
                    stored: bindings[2].logical_path.clone(),
                    found: job.join(r"02 - Templates\_qq\QQ\P00084_I.jpg"),
                },
            ]
        );
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "only paths under the copy are touched, never the old server: {elapsed:?}"
        );
    }

    #[test]
    fn the_copy_beside_the_project_wins_over_a_stored_path_that_still_exists() {
        let root = tempfile::tempdir().unwrap();
        let original = root
            .path()
            .join(r"Servidor\J040\02 - Templates\MOLDURA.png");
        let project_path = root
            .path()
            .join(r"Copia\J040\02 - Templates\Modelo.myalbuns");
        write(&original);
        write(&project_path);
        write(&project_path.with_file_name("MOLDURA.png"));

        let (found, _) = found(
            &project_path,
            &[binding("frame", MediaKind::Decorative, &original)],
        );

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].found, project_path.with_file_name("MOLDURA.png"));
    }

    #[test]
    fn a_path_already_linked_by_another_media_of_the_same_kind_is_not_taken() {
        let root = tempfile::tempdir().unwrap();
        let project_path = root.path().join(r"J040\02 - Templates\Modelo.myalbuns");
        let beside = project_path.with_file_name("AB.png");
        write(&project_path);
        write(&beside);
        let stored = Path::new(UNREACHABLE).join(r"J040\02 - Templates\AB.png");

        let (found, _) = found(
            &project_path,
            &[
                binding("old", MediaKind::Decorative, &stored),
                binding("current", MediaKind::Decorative, &beside),
                binding("photo", MediaKind::Photo, &stored),
            ],
        );

        assert_eq!(
            found,
            [FoundMedia {
                media_id: "photo".into(),
                stored,
                found: beside,
            }],
            "a Photo may share its file with a Decorative, as in the Project file"
        );
    }

    #[test]
    fn a_project_without_matching_folders_finds_nothing() {
        let root = tempfile::tempdir().unwrap();
        let project_path = root.path().join(r"Outro\Modelo.myalbuns");
        write(&project_path);
        write(&project_path.with_file_name("MOLDURA.png"));

        let (found, _) = found(
            &project_path,
            &[binding(
                "frame",
                MediaKind::Decorative,
                Path::new(UNREACHABLE).join(r"J040\02 - Templates\MOLDURA.png"),
            )],
        );

        assert!(found.is_empty(), "a file with the same name is not enough");
    }
}
