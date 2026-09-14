use crate::guarded_fs::{DirectoryGuard, GuardedFsError, ensure_direct_child, open_directory};
use crate::{ExpectedObject, PhysicalIdentityEvidence, RootBindingPlan};
use std::{
    fmt,
    path::{Component, Path},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirroredDestinationError {
    SourceUnavailable,
    DestinationUnavailable,
    InsideSource,
    RelationshipIndeterminate,
    InvalidHierarchy,
    StorageFull,
}
impl fmt::Display for MirroredDestinationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SourceUnavailable => "A pasta de origem está indisponível.",
            Self::DestinationUnavailable => "A pasta de destino está indisponível.",
            Self::InsideSource => "Escolha um destino fora da pasta de origem.",
            Self::RelationshipIndeterminate => {
                "Não foi possível confirmar que origem e destino são independentes."
            }
            Self::InvalidHierarchy => {
                "A hierarquia de destino está indisponível ou aponta para fora da pasta escolhida."
            }
            Self::StorageFull => "Não há espaço para criar a pasta de destino.",
        })
    }
}
impl std::error::Error for MirroredDestinationError {}

/// Retains the selected roots while a mirrored hierarchy is inspected or created.
pub struct MirroredDestination {
    _source: DirectoryGuard,
    destination: DirectoryGuard,
}
/// Keeps every ancestor of a generated Project immovable until its publication ends.
pub struct MirroredParent {
    _directories: Vec<DirectoryGuard>,
}

impl MirroredDestination {
    /// Checks the existing hierarchy without creating missing directories.
    pub fn inspect_parent(&self, relative: &Path) -> Result<(), MirroredDestinationError> {
        validate_relative(relative)?;
        let mut directories = Vec::new();
        for name in relative.iter() {
            let parent = directories.last().unwrap_or(&self.destination);
            let child = parent.logical_path.join(name);
            crate::validate_external_path(&child)
                .map_err(|_| MirroredDestinationError::InvalidHierarchy)?;
            match crate::guarded_fs::open_existing_direct_child(parent, &child) {
                Ok(Some(directory)) => directories.push(directory),
                Ok(None) => return Ok(()),
                Err(_) => return Err(MirroredDestinationError::InvalidHierarchy),
            }
        }
        Ok(())
    }
    pub fn open(
        plan: &RootBindingPlan,
        source: &Path,
        destination: &Path,
    ) -> Result<Self, MirroredDestinationError> {
        let source = plan
            .resolve_existing(source, ExpectedObject::Directory)
            .map_err(|_| MirroredDestinationError::SourceUnavailable)?;
        let destination = plan
            .resolve_existing(destination, ExpectedObject::Directory)
            .map_err(|_| MirroredDestinationError::DestinationUnavailable)?;
        let source = open_directory(source.operational_path())
            .map_err(|_| MirroredDestinationError::SourceUnavailable)?;
        let destination = open_directory(destination.operational_path())
            .map_err(|_| MirroredDestinationError::DestinationUnavailable)?;
        // Handle identities also cover mapped drives and alternate UNC spellings.
        for ancestor in destination
            .physical_path
            .ancestors()
            .take_while(|path| path.has_root())
        {
            let ancestor = open_directory(ancestor)
                .map_err(|_| MirroredDestinationError::RelationshipIndeterminate)?;
            match crate::resolve::compare_file_identity(
                &source.containment_handle,
                &ancestor.containment_handle,
            ) {
                PhysicalIdentityEvidence::Same => {
                    return Err(MirroredDestinationError::InsideSource);
                }
                PhysicalIdentityEvidence::Different => {}
                PhysicalIdentityEvidence::Indeterminate => {
                    return Err(MirroredDestinationError::RelationshipIndeterminate);
                }
            }
        }
        Ok(Self {
            _source: source,
            destination,
        })
    }
    pub fn prepare_parent(
        &self,
        relative: &Path,
    ) -> Result<MirroredParent, MirroredDestinationError> {
        validate_relative(relative)?;
        let mut directories = Vec::new();
        for name in relative.iter() {
            let parent = directories.last().unwrap_or(&self.destination);
            let child = parent.logical_path.join(name);
            crate::validate_external_path(&child)
                .map_err(|_| MirroredDestinationError::InvalidHierarchy)?;
            directories.push(
                ensure_direct_child(parent, &child).map_err(|error| match error {
                    GuardedFsError::StorageFull => MirroredDestinationError::StorageFull,
                    _ => MirroredDestinationError::InvalidHierarchy,
                })?,
            );
        }
        Ok(MirroredParent {
            _directories: directories,
        })
    }
}
fn validate_relative(relative: &Path) -> Result<(), MirroredDestinationError> {
    if relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        Ok(())
    } else {
        Err(MirroredDestinationError::InvalidHierarchy)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::OperationPathContext;
    use std::os::windows::process::CommandExt;
    fn link(link: &Path, target: &Path) {
        let output = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn plan(source: &Path, target: &Path) -> RootBindingPlan {
        let mut paths = OperationPathContext::new();
        paths.capture(source).unwrap();
        paths.capture(target).unwrap();
        paths.freeze()
    }
    #[test]
    fn an_alias_of_source_or_its_descendant_cannot_hide_containment() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let alias = root.path().join("alias");
        std::fs::create_dir_all(source.join("nested")).unwrap();
        link(&alias, &source);
        for target in [&alias, &alias.join("nested")] {
            assert!(matches!(
                MirroredDestination::open(&plan(&source, target), &source, target),
                Err(MirroredDestinationError::InsideSource)
            ));
        }
        std::fs::remove_dir(alias).unwrap();
    }
    #[test]
    fn existing_junctions_and_relative_escapes_are_rejected_without_creating_directories() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        let outside = root.path().join("outside");
        for folder in [&source, &target, &outside] {
            std::fs::create_dir(folder).unwrap();
        }
        let junction = target.join("Turma");
        link(&junction, &outside);
        let mirror = MirroredDestination::open(&plan(&source, &target), &source, &target).unwrap();
        assert_eq!(
            mirror.inspect_parent(Path::new("Turma/001")),
            Err(MirroredDestinationError::InvalidHierarchy)
        );
        assert_eq!(
            mirror.inspect_parent(Path::new("missing/../escape")),
            Err(MirroredDestinationError::InvalidHierarchy)
        );
        assert!(mirror.prepare_parent(Path::new("Turma/001")).is_err());
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
        assert!(!target.join("missing").exists());
        drop(mirror);
        std::fs::remove_dir(junction).unwrap();
    }
}
