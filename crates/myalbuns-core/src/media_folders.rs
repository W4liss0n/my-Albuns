use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{CoreError, MediaId, MediaKind, ProjectDocument};

/// Ordered, project-owned organization; each media belongs to at most one folder.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaFolder {
    pub id: String,
    pub kind: MediaKind,
    pub name: String,
    #[ts(type = "Array<string>")]
    pub media_ids: Vec<MediaId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(tag = "kind")]
pub enum MediaFolderEdit {
    Create {
        media_kind: MediaKind,
        name: String,
    },
    Rename {
        folder_id: String,
        name: String,
    },
    Delete {
        folder_id: String,
    },
    MoveMedia {
        #[ts(type = "Array<string>")]
        media_ids: Vec<MediaId>,
        folder_id: Option<String>,
    },
}

fn invalid(message: &str) -> CoreError {
    CoreError::InvalidProject(message.into())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaFolderNameRequest {
    pub media_kind: MediaKind,
    pub name: String,
    pub folder_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MediaFolderNameError {
    Empty,
    InvalidCharactersOrLength,
    NameInUse,
    FolderNotFound,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaFolderNameValidation {
    pub name: String,
    pub error: Option<MediaFolderNameError>,
}

fn name_error(name: &str, name_in_use: bool) -> Option<MediaFolderNameError> {
    if name.is_empty() {
        Some(MediaFolderNameError::Empty)
    } else if name.chars().count() > 80 || name.chars().any(char::is_control) {
        Some(MediaFolderNameError::InvalidCharactersOrLength)
    } else if name_in_use || ["todas", "ausentes"].contains(&name.to_lowercase().as_str()) {
        Some(MediaFolderNameError::NameInUse)
    } else {
        None
    }
}

impl ProjectDocument {
    /// Read-only advice; edits revalidate against the document at execution time.
    pub fn validate_media_folder_name(
        &self,
        request: &MediaFolderNameRequest,
    ) -> MediaFolderNameValidation {
        let name = request.name.trim().to_owned();
        let error = if request.folder_id.as_ref().is_some_and(|id| {
            !self
                .media_folders
                .iter()
                .any(|folder| &folder.id == id && folder.kind == request.media_kind)
        }) {
            Some(MediaFolderNameError::FolderNotFound)
        } else {
            name_error(
                &name,
                self.media_folders.iter().any(|folder| {
                    folder.kind == request.media_kind
                        && Some(&folder.id) != request.folder_id.as_ref()
                        && folder.name.to_lowercase() == name.to_lowercase()
                }),
            )
        };
        MediaFolderNameValidation { name, error }
    }

    pub fn media_folders(&self) -> &[MediaFolder] {
        &self.media_folders
    }

    pub(crate) fn media_folders_are_valid(&self) -> bool {
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        let mut members = HashSet::new();
        let media_kinds: HashMap<_, _> = self
            .media()
            .iter()
            .map(|media| (media.id(), media.kind()))
            .collect();
        self.media_folders.iter().all(|folder| {
            Uuid::parse_str(&folder.id)
                .is_ok_and(|id| id.get_version_num() == 4 && id.to_string() == folder.id)
                && ids.insert(&folder.id)
                && folder.name == folder.name.trim()
                && name_error(
                    &folder.name,
                    !names.insert((folder.kind, folder.name.to_lowercase())),
                )
                .is_none()
                && folder.media_ids.iter().all(|id| {
                    members.insert(*id) && media_kinds.get(&id.into_uuid()) == Some(&folder.kind)
                })
        })
    }

    pub(crate) fn restore_media_folders(mut self, folders: Vec<MediaFolder>) -> Result<Self, ()> {
        self.media_folders = folders;
        if !self.media_folders_are_valid() {
            return Err(());
        }
        Ok(self)
    }

    pub(crate) fn with_media_folder_edit(&self, edit: &MediaFolderEdit) -> Result<Self, CoreError> {
        let mut next = self.clone();
        match edit {
            MediaFolderEdit::Create { media_kind, name } => {
                next.media_folders.push(MediaFolder {
                    id: Uuid::new_v4().to_string(),
                    kind: *media_kind,
                    name: name.trim().into(),
                    media_ids: Vec::new(),
                });
            }
            MediaFolderEdit::Rename { folder_id, name } => {
                next.media_folders
                    .iter_mut()
                    .find(|folder| &folder.id == folder_id)
                    .ok_or_else(|| invalid("A pasta não existe mais."))?
                    .name = name.trim().into();
            }
            MediaFolderEdit::Delete { folder_id } => {
                if !next
                    .media_folders
                    .iter()
                    .any(|folder| &folder.id == folder_id)
                {
                    return Err(invalid("A pasta não existe mais."));
                }
                next.media_folders.retain(|folder| &folder.id != folder_id);
            }
            MediaFolderEdit::MoveMedia {
                media_ids,
                folder_id,
            } => {
                let mut kind = None;
                for id in media_ids {
                    let media = self
                        .media()
                        .iter()
                        .find(|media| media.id() == id.into_uuid())
                        .ok_or_else(|| CoreError::MediaNotFound(id.to_string()))?;
                    if kind.is_some_and(|kind| kind != media.kind()) {
                        return Err(invalid("Selecione imagens de uma única aba."));
                    }
                    kind = Some(media.kind());
                }
                if let Some(id) = folder_id {
                    let folder = self
                        .media_folders
                        .iter()
                        .find(|folder| &folder.id == id)
                        .ok_or_else(|| invalid("A pasta não existe mais."))?;
                    if kind.is_some_and(|kind| kind != folder.kind) {
                        return Err(invalid("A pasta pertence a outra aba."));
                    }
                }
                let selected = media_ids.iter().copied().collect::<HashSet<_>>();
                for folder in &mut next.media_folders {
                    if Some(&folder.id) == folder_id.as_ref() {
                        // Keep existing member order, including on a no-op move.
                        for id in media_ids {
                            if !folder.media_ids.contains(id) {
                                folder.media_ids.push(*id);
                            }
                        }
                    } else {
                        folder.media_ids.retain(|id| !selected.contains(id));
                    }
                }
            }
        }
        if !next.media_folders_are_valid() {
            return Err(invalid(
                "Use um nome de até 80 caracteres, diferente de Todas, Ausentes e das outras pastas desta aba.",
            ));
        }
        Ok(next)
    }
}
