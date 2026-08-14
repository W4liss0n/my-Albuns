use std::path::{Path, PathBuf};

use myalbuns_paths::AppPaths;
use tracing_appender::{
    non_blocking::{NonBlockingBuilder, WorkerGuard},
    rolling::{Builder, Rotation},
};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub const LOG_DIRECTORY_ENV: &str = "MYALBUNS_LOG_DIR";
pub const LOG_FILTER_ENV: &str = "MYALBUNS_LOG";

const MAX_LOG_FILES: usize = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessRole {
    Global,
    DesktopHost,
    Imaging,
}

impl ProcessRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::DesktopHost => "desktop_host",
            Self::Imaging => "imaging",
        }
    }

    const fn file_prefix(self) -> &'static str {
        match self {
            Self::Global => "myalbuns-global",
            Self::DesktopHost => "myalbuns-desktop",
            Self::Imaging => "myalbuns-imaging",
        }
    }
}

#[derive(Debug)]
pub struct LoggingGuard {
    _file_guard: WorkerGuard,
}

pub fn init_local_logging(
    log_directory: &Path,
    process_role: ProcessRole,
) -> Result<LoggingGuard, String> {
    std::fs::create_dir_all(log_directory)
        .map_err(|error| format!("não foi possível criar o diretório de logs: {error}"))?;
    let file_appender = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix(process_role.file_prefix())
        .filename_suffix("jsonl")
        .max_log_files(MAX_LOG_FILES)
        .build(log_directory)
        .map_err(|error| format!("não foi possível preparar o arquivo de log: {error}"))?;
    let (file_writer, file_guard) = NonBlockingBuilder::default()
        .lossy(false)
        .finish(file_appender);
    let filter = EnvFilter::try_from_env(LOG_FILTER_ENV).unwrap_or_else(|_| {
        EnvFilter::new(if cfg!(debug_assertions) {
            "myalbuns=debug"
        } else {
            "myalbuns=info"
        })
    });
    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .flatten_event(true)
        .with_ansi(false)
        .with_writer(file_writer);

    #[cfg(debug_assertions)]
    let initialization = if process_role != ProcessRole::Imaging {
        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .with(
                tracing_subscriber::fmt::layer()
                    .compact()
                    .with_ansi(false)
                    .with_writer(std::io::stderr),
            )
            .try_init()
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .try_init()
    };

    #[cfg(not(debug_assertions))]
    let initialization = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .try_init();

    initialization
        .map_err(|error| format!("não foi possível instalar o subscriber de logs: {error}"))?;
    Ok(LoggingGuard {
        _file_guard: file_guard,
    })
}

/// Resolves the directory supplied by the host to the Processador de Imagens.
///
/// The host uses `AppPaths::logs_dir()` directly and sets this override when it
/// starts the sidecar. Local discovery is only the fallback for an independent
/// Processador execution.
pub fn sidecar_log_directory(app_paths: &AppPaths) -> PathBuf {
    std::env::var_os(LOG_DIRECTORY_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| app_paths.logs_dir())
}

pub fn safe_log_identifier(value: &str) -> Option<&str> {
    (!value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte)))
    .then_some(value)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use myalbuns_paths::AppPaths;

    use super::{LOG_DIRECTORY_ENV, ProcessRole, safe_log_identifier, sidecar_log_directory};

    #[test]
    fn process_roles_have_stable_distinct_log_identities() {
        assert_eq!(ProcessRole::Global.as_str(), "global");
        assert_eq!(ProcessRole::DesktopHost.as_str(), "desktop_host");
        assert_eq!(ProcessRole::Imaging.as_str(), "imaging");
        assert_ne!(
            ProcessRole::Global.file_prefix(),
            ProcessRole::DesktopHost.file_prefix()
        );
        assert_ne!(
            ProcessRole::Global.file_prefix(),
            ProcessRole::Imaging.file_prefix()
        );
        assert_ne!(
            ProcessRole::DesktopHost.file_prefix(),
            ProcessRole::Imaging.file_prefix()
        );
    }

    #[test]
    fn sidecar_directory_defaults_to_the_central_application_paths() {
        if std::env::var_os(LOG_DIRECTORY_ENV).is_none() {
            let app_paths = AppPaths::from_roots(Path::new(r"C:\Roaming"), Path::new(r"C:\Local"));

            assert_eq!(
                sidecar_log_directory(&app_paths),
                Path::new(r"C:\Local\MyAlbuns2\Logs")
            );
        }
    }

    #[test]
    fn log_identifiers_cannot_carry_paths_or_forged_events() {
        assert_eq!(
            safe_log_identifier("project-01.ABC"),
            Some("project-01.ABC")
        );
        assert_eq!(safe_log_identifier(r"c:\users\person\album"), None);
        assert_eq!(safe_log_identifier("project\nforged-event"), None);
        assert_eq!(safe_log_identifier("Álbum da família"), None);
    }
}
