//! Cross-host batch ownership and safe-point acknowledgement. Kernel ownership
//! makes both admission and cache pauses recover when a process disappears.
use std::{path::PathBuf, time::Duration};

use myalbuns_paths::AppPaths;
use tauri::{AppHandle, Manager};

use crate::{
    batch_runner::BatchCancellation,
    named_mutex::{NamedMutex, NamedMutexGrant},
};

pub(crate) fn gate(paths: &AppPaths) -> NamedMutex {
    NamedMutex::scoped(paths, "batch-mode", "application", "batch-mode-owner")
}

fn participant_gate(paths: &AppPaths, id: &str, kind: &str) -> NamedMutex {
    NamedMutex::scoped(paths, kind, id, "batch-participant")
}

struct Participant {
    path: PathBuf,
    _alive: NamedMutexGrant,
    paused: NamedMutex,
}

impl Participant {
    fn register(paths: &AppPaths) -> Result<Self, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let alive = participant_gate(paths, &id, "batch-participant-alive")
            .try_acquire()
            .map_err(|error| format!("Registro de Projeto indisponível: {error:?}"))?;
        let directory = paths.state_dir().join("BatchParticipants");
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let path = directory.join(&id);
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            path,
            _alive: alive,
            paused: participant_gate(paths, &id, "batch-participant-paused"),
        })
    }
}

impl Drop for Participant {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Installed before the Project Host can start any image/cache work.
pub(crate) fn install_project(app: &AppHandle, paths: &AppPaths) -> Result<(), String> {
    let participant = Participant::register(paths)?;
    let owner = gate(paths);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            if owner.is_owned().unwrap_or(true) {
                let operations = app.state::<crate::project_ui_operations::ProjectUiOperations>();
                let Ok(_admission) = operations.pause_for_batch() else {
                    tokio::time::sleep(Duration::from_millis(40)).await;
                    continue;
                };
                // Previously admitted commands must finish before the global
                // owner receives the acknowledgement; new commands are denied.
                while owner.is_owned().unwrap_or(true) && !operations.is_idle() {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                if !owner.is_owned().unwrap_or(true) {
                    continue;
                }
                let cache = app.state::<crate::cache_engine::CacheEngine>();
                let pause = cache.pause().await;
                let processor = app.state::<crate::imaging_processor::ImagingProcessor>();
                let reservation = processor.reserve().await;
                if reservation.is_err()
                    || crate::application_modality::synchronize(&app)
                        .await
                        .is_err()
                {
                    while owner.is_owned().unwrap_or(true) {
                        tokio::time::sleep(Duration::from_millis(40)).await;
                    }
                    continue;
                }
                let acknowledgement = loop {
                    match participant.paused.try_acquire() {
                        Ok(grant) => break Some(grant),
                        Err(_) if !owner.is_owned().unwrap_or(true) => break None,
                        Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
                    }
                };
                while owner.is_owned().unwrap_or(true) {
                    tokio::time::sleep(Duration::from_millis(40)).await;
                }
                drop(acknowledgement);
                drop(reservation);
                drop(pause);
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    });
    Ok(())
}

pub(crate) async fn acquire(
    paths: &AppPaths,
    cancel: &BatchCancellation,
) -> Result<NamedMutexGrant, String> {
    let observation_deadline = std::time::Instant::now() + Duration::from_secs(2);
    let reservation = loop {
        match gate(paths).try_acquire() {
            Ok(reservation) => break reservation,
            Err(crate::named_mutex::NamedMutexError::Conflict)
                if std::time::Instant::now() < observation_deadline && !cancel.is_requested() =>
            {
                tokio::time::sleep(Duration::from_millis(10)).await
            }
            Err(error) => {
                return Err(format!(
                    "Não foi possível iniciar o modo de lote: {error:?}"
                ));
            }
        }
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        if cancel.is_requested() {
            return Err("Exportação cancelada.".into());
        }
        if participants_ready(paths)? {
            return Ok(reservation);
        }
        if std::time::Instant::now() >= deadline {
            return Err(
                "Um Álbum ainda está ocupado. Conclua a operação aberta e tente novamente.".into(),
            );
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
}

fn participants_ready(paths: &AppPaths) -> Result<bool, String> {
    for id in live_participants(paths)?.0 {
        if !participant_gate(paths, &id, "batch-participant-paused")
            .is_owned()
            .map_err(|error| format!("Não foi possível pausar um Álbum: {error:?}"))?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Removes the registrations of Project Hosts that exited without closing,
/// so the folder does not keep one file per process that ever crashed.
pub(crate) fn prune_closed_participants(paths: &AppPaths) -> usize {
    live_participants(paths).map_or(0, |(_, removed)| removed)
}

/// Lists live participants and removes the registrations whose process is
/// gone. A live participant owns its mutex before publishing its unique UUID.
fn live_participants(paths: &AppPaths) -> Result<(Vec<String>, usize), String> {
    let entries = match std::fs::read_dir(paths.state_dir().join("BatchParticipants")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), 0));
        }
        Err(error) => return Err(error.to_string()),
    };
    let mut live = Vec::new();
    let mut removed = 0;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        let Some(id) = name.to_str().filter(|id| uuid::Uuid::parse_str(id).is_ok()) else {
            continue;
        };
        let alive = participant_gate(paths, id, "batch-participant-alive")
            .is_owned()
            .map_err(|error| format!("Não foi possível verificar um Álbum aberto: {error:?}"))?;
        if alive {
            live.push(id.to_owned());
        } else if std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    Ok((live, removed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_live_hosts_and_ignores_closed_participants() {
        tauri::async_runtime::block_on(async {
            let root = tempfile::tempdir().unwrap();
            let paths = AppPaths::from_roots(root.path(), root.path());
            let host = Participant::register(&paths).unwrap();
            let cancel = BatchCancellation::default();
            let attempt = acquire(&paths, &cancel);
            tokio::pin!(attempt);
            assert!(
                tokio::time::timeout(Duration::from_millis(80), &mut attempt)
                    .await
                    .is_err()
            );
            assert!(gate(&paths).is_owned().unwrap());
            let pause = host.paused.try_acquire().unwrap();
            let reservation = attempt.await.unwrap();
            drop(reservation);
            assert!(!gate(&paths).is_owned().unwrap());
            drop(pause);
            // Simulate a stale registration left by a process exit.
            let stale_path = host.path.clone();
            drop(host);
            std::fs::write(stale_path, b"").unwrap();
            acquire(&paths, &cancel).await.unwrap();
        });
    }

    #[test]
    fn startup_removes_only_registrations_of_exited_hosts() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(root.path(), root.path());
        let live = Participant::register(&paths).unwrap();
        let exited = Participant::register(&paths).unwrap();
        let exited_path = exited.path.clone();
        drop(exited);
        std::fs::write(&exited_path, b"").unwrap();

        assert_eq!(prune_closed_participants(&paths), 1);

        assert!(!exited_path.exists());
        assert!(live.path.exists());
    }
}
