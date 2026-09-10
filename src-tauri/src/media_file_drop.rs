use crate::ipc_contract::MediaFileDrag;
use std::{path::PathBuf, sync::Mutex};

pub(crate) const MEDIA_FILE_DRAG_EVENT: &str = "myalbuns-media-file-drag";

struct PendingDrop {
    id: String,
    paths: Vec<PathBuf>,
}

/// At most one unclaimed drop belongs to this Project window. Paths stay native;
/// the WebView only decides whether the drop landed inside the visible Panel.
#[derive(Default)]
pub(crate) struct NativeMediaDrops(Mutex<Option<PendingDrop>>);

impl NativeMediaDrops {
    pub(crate) fn receive(&self, event: &tauri::DragDropEvent) -> Option<MediaFileDrag> {
        match event {
            tauri::DragDropEvent::Enter { position, .. } => {
                *self.0.lock().ok()? = None;
                Some(MediaFileDrag::Over {
                    x: position.x,
                    y: position.y,
                })
            }
            tauri::DragDropEvent::Over { position } => Some(MediaFileDrag::Over {
                x: position.x,
                y: position.y,
            }),
            tauri::DragDropEvent::Drop { paths, position } => {
                let drop_id = uuid::Uuid::new_v4().to_string();
                *self.0.lock().ok()? = Some(PendingDrop {
                    id: drop_id.clone(),
                    paths: paths.clone(),
                });
                Some(MediaFileDrag::Drop {
                    x: position.x,
                    y: position.y,
                    drop_id,
                })
            }
            // A native Leave can follow Drop before the async import claims it.
            tauri::DragDropEvent::Leave => Some(MediaFileDrag::Leave),
            _ => None,
        }
    }

    pub(crate) fn take(&self, drop_id: &str) -> Result<Vec<PathBuf>, String> {
        let mut pending = self
            .0
            .lock()
            .map_err(|_| "Não foi possível receber os arquivos arrastados.".to_string())?;
        if pending.as_ref().is_none_or(|pending| pending.id != drop_id) {
            return Err(
                "Esta soltura não está mais disponível. Arraste os arquivos novamente.".into(),
            );
        }
        Ok(pending.take().expect("the matching drop exists").paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};

    #[test]
    fn native_drop_keeps_non_unicode_windows_paths_outside_json_and_consumes_once() {
        let path = PathBuf::from(OsString::from_wide(&[
            67, 58, 92, 70, 111, 116, 111, 115, 92, 0xd800, 46, 112, 110, 103,
        ]));
        let drops = NativeMediaDrops::default();
        let event = drops
            .receive(&tauri::DragDropEvent::Drop {
                paths: vec![path.clone()],
                position: tauri::PhysicalPosition::new(30.0, 60.0),
            })
            .unwrap();
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["kind"], "drop");
        assert!(json.get("paths").is_none());
        let id = json["dropId"].as_str().unwrap();
        assert_eq!(drops.take(id).unwrap(), vec![path]);
        assert!(drops.take(id).is_err());
        assert!(drops.take("an unrelated drop").is_err());
    }
}
