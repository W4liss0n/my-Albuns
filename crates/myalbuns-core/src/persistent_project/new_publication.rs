use std::path::Path;

use myalbuns_paths::PhysicalFileIdentity;

use super::{
    CreateAuthorization, ProjectCore, ProjectIdentityAuthority, SaveAsAuthorization,
    bind_identity_target, publish_identity_location,
};
use crate::{
    project_document::ProjectRevision,
    project_store::{
        self, CreateStoreError, IdentityLeaseError, PathFailure, ProjectIdentityLease,
        ProjectLocation, ProjectStore,
    },
};

/// Public operations keep their distinct inputs. This private target only
/// carries the publication authorization and the source that Save As protects.
pub(super) enum NewProjectTarget<'a> {
    Create {
        location: ProjectLocation,
        authorization: CreateAuthorization,
    },
    SaveAs {
        location: ProjectLocation,
        authorization: SaveAsAuthorization,
        source: &'a ProjectStore,
        source_identity: PhysicalFileIdentity,
    },
}

pub(super) struct PublishedNewProject {
    pub(super) store: ProjectStore,
    pub(super) identity_lease: ProjectIdentityLease,
    pub(super) identity_authority: ProjectIdentityAuthority,
}

/// Preserve the stage at which authority could not be established: the public
/// operations intentionally expose different indeterminate terminals.
pub(super) enum NewPublicationError {
    Store(CreateStoreError),
    BaselineChanged,
    RegistryUnavailable,
    BindingUnavailable,
}

impl NewProjectTarget<'_> {
    fn publish(
        self,
        revision: &ProjectRevision,
        lease_root: &Path,
    ) -> Result<ProjectStore, CreateStoreError> {
        let prepared = match self {
            Self::Create {
                location,
                authorization: CreateAuthorization::CreateOnly,
            } => return project_store::create_only(location, revision, lease_root),
            Self::Create {
                location,
                authorization: CreateAuthorization::ReplaceConfirmed,
            } => project_store::prepare_replacement(location, revision, lease_root),
            Self::Create {
                location,
                authorization: CreateAuthorization::ReplaceTargetConfirmed(identity),
            } => project_store::prepare_replacement_confirmed(
                location, revision, lease_root, identity,
            ),
            Self::SaveAs {
                location,
                authorization: SaveAsAuthorization::CreateOnly,
                source_identity,
                ..
            } => {
                return project_store::create_only_excluding(
                    location,
                    revision,
                    lease_root,
                    source_identity,
                );
            }
            Self::SaveAs {
                location,
                authorization: SaveAsAuthorization::ReplaceConfirmed(confirmed_target),
                source_identity,
                ..
            } => project_store::prepare_replacement_excluding(
                location,
                revision,
                lease_root,
                source_identity,
                confirmed_target,
            ),
        }?;
        let _replaced_identity_lease = prepared
            .replaced_project_id()
            .map(|id| ProjectIdentityLease::acquire(lease_root, id))
            .transpose()
            .map_err(|error| match error {
                IdentityLeaseError::Conflict => CreateStoreError::ProjectInUse,
                IdentityLeaseError::Unavailable => CreateStoreError::Path(PathFailure::IoFailure),
            })?;
        prepared.publish()
    }
}

pub(super) fn publish_new_project(
    core: &ProjectCore,
    revision: &ProjectRevision,
    target: NewProjectTarget<'_>,
) -> Result<PublishedNewProject, NewPublicationError> {
    let lease_root = core
        .identity_lease_root()
        .ok_or(NewPublicationError::Store(CreateStoreError::Path(
            PathFailure::IoFailure,
        )))?;
    let identity_lease =
        ProjectIdentityLease::acquire(lease_root, revision.project_id).map_err(|error| {
            NewPublicationError::Store(match error {
                IdentityLeaseError::Conflict => CreateStoreError::IdentityIndeterminate,
                IdentityLeaseError::Unavailable => CreateStoreError::Path(PathFailure::IoFailure),
            })
        })?;
    let source = match &target {
        NewProjectTarget::Create { .. } => None,
        NewProjectTarget::SaveAs { source, .. } => Some(*source),
    };
    let store = match target.publish(revision, lease_root) {
        Ok(store) => store,
        Err(error) => {
            identity_lease.discard_unpublished();
            return Err(NewPublicationError::Store(error));
        }
    };
    if !store.location_still_matches_baseline()
        || source.is_some_and(|source| !source.location_still_matches_baseline())
    {
        identity_lease.discard_unpublished();
        return Err(NewPublicationError::BaselineChanged);
    }
    if publish_identity_location(core, revision.project_id, &store).is_err() {
        identity_lease.discard_unpublished();
        return Err(NewPublicationError::RegistryUnavailable);
    }
    let identity_lease = bind_identity_target(identity_lease, &store)
        .map_err(|_| NewPublicationError::BindingUnavailable)?;
    Ok(PublishedNewProject {
        store,
        identity_authority: ProjectIdentityAuthority::authorized(identity_lease.project_id()),
        identity_lease,
    })
}
