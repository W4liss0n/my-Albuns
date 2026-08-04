use std::io::{Read, Write};

use myalbuns_paths::{NativePathDto, RootBindingPlan};
use serde::{Deserialize, Serialize};

pub(crate) const PROTOCOL_VERSION: u16 = 1;
const MAX_BOOTSTRAP_REQUEST_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum BootstrapIntent {
    OpenExisting,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TargetAuthority {
    pub(crate) logical_target: NativePathDto,
    pub(crate) root_bindings: RootBindingPlan,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BootstrapRequest {
    pub(crate) protocol_version: u16,
    pub(crate) attempt_id: String,
    pub(crate) launch_nonce: String,
    pub(crate) intent: BootstrapIntent,
    pub(crate) authority: TargetAuthority,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FailureStage {
    Decode,
    Resolve,
    Open,
    Initialize,
    Transport,
    Protocol,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FailureCode {
    InvalidRequest,
    NotFound,
    Unavailable,
    AccessDenied,
    InvalidPath,
    UnexpectedObjectType,
    Conflict,
    IoFailure,
    InvalidDocumentType,
    UnsupportedFutureSchema,
    UnsupportedLegacySchema,
    InvalidProjectDocument,
    InvalidProjectState,
    ProjectInUse,
    ExternalCopyRequiresInteractiveResolution,
    IdentityIndeterminate,
    HostExitedBeforeReady,
    CorrelationMismatch,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "state"
)]
pub(crate) enum HostTerminal {
    Ready {
        attempt_id: String,
        launch_nonce: String,
        host_pid: u32,
        project_id: String,
        revision: u64,
    },
    Failed {
        attempt_id: String,
        launch_nonce: String,
        host_pid: u32,
        stage: FailureStage,
        code: FailureCode,
    },
}

impl HostTerminal {
    pub(crate) fn ready(request: &BootstrapRequest, project_id: String, revision: u64) -> Self {
        Self::Ready {
            attempt_id: request.attempt_id.clone(),
            launch_nonce: request.launch_nonce.clone(),
            host_pid: std::process::id(),
            project_id,
            revision,
        }
    }

    pub(crate) fn failed(
        request: &BootstrapRequest,
        stage: FailureStage,
        code: FailureCode,
    ) -> Self {
        Self::Failed {
            attempt_id: request.attempt_id.clone(),
            launch_nonce: request.launch_nonce.clone(),
            host_pid: std::process::id(),
            stage,
            code,
        }
    }

    pub(crate) fn uncorrelated_failure(stage: FailureStage, code: FailureCode) -> Self {
        Self::Failed {
            attempt_id: String::new(),
            launch_nonce: String::new(),
            host_pid: std::process::id(),
            stage,
            code,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ValidatedTerminal {
    Ready {
        host_pid: u32,
        project_id: String,
        revision: u64,
    },
    Failed {
        host_pid: u32,
        stage: FailureStage,
        code: FailureCode,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalValidationError {
    CorrelationMismatch,
}

pub(crate) fn validate_terminal(
    request: &BootstrapRequest,
    spawned_pid: u32,
    terminal: HostTerminal,
) -> Result<ValidatedTerminal, TerminalValidationError> {
    let (attempt_id, launch_nonce, host_pid) = match &terminal {
        HostTerminal::Ready {
            attempt_id,
            launch_nonce,
            host_pid,
            ..
        }
        | HostTerminal::Failed {
            attempt_id,
            launch_nonce,
            host_pid,
            ..
        } => (attempt_id, launch_nonce, *host_pid),
    };
    if attempt_id != &request.attempt_id
        || launch_nonce != &request.launch_nonce
        || host_pid != spawned_pid
    {
        return Err(TerminalValidationError::CorrelationMismatch);
    }

    Ok(match terminal {
        HostTerminal::Ready {
            host_pid,
            project_id,
            revision,
            ..
        } => ValidatedTerminal::Ready {
            host_pid,
            project_id,
            revision,
        },
        HostTerminal::Failed {
            host_pid,
            stage,
            code,
            ..
        } => ValidatedTerminal::Failed {
            host_pid,
            stage,
            code,
        },
    })
}

pub(crate) fn read_bootstrap_request(
    reader: impl Read,
) -> Result<BootstrapRequest, std::io::Error> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_BOOTSTRAP_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BOOTSTRAP_REQUEST_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "a requisição de bootstrap excede o limite",
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("requisição de bootstrap inválida: {error}"),
        )
    })
}

pub(crate) fn write_host_terminal(
    mut writer: impl Write,
    terminal: &HostTerminal,
) -> Result<(), std::io::Error> {
    serde_json::to_writer(&mut writer, terminal).map_err(std::io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use myalbuns_paths::OperationPathContext;

    use super::*;

    fn request() -> BootstrapRequest {
        let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Projeto.myalbuns");
        let mut paths = OperationPathContext::new();
        paths
            .capture(&target)
            .expect("the fixture path has a supported root");
        BootstrapRequest {
            protocol_version: PROTOCOL_VERSION,
            attempt_id: "attempt-1".into(),
            launch_nonce: "nonce-1".into(),
            intent: BootstrapIntent::OpenExisting,
            authority: TargetAuthority {
                logical_target: NativePathDto::from(target),
                root_bindings: paths.freeze(),
            },
        }
    }

    #[test]
    fn request_round_trip_preserves_native_authority_without_a_unicode_path_string() {
        let request = request();
        let encoded = serde_json::to_value(&request).expect("the request serializes");

        assert_eq!(encoded["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(encoded["intent"], "openExisting");
        assert!(
            encoded["authority"]["logicalTarget"]
                .get("encoding")
                .is_some()
        );
        assert!(!encoded["authority"]["logicalTarget"].is_string());

        let decoded: BootstrapRequest =
            serde_json::from_value(encoded).expect("the request deserializes");
        assert_eq!(decoded, request);
    }

    #[cfg(windows)]
    #[test]
    fn bootstrap_wire_preserves_every_supported_windows_path_form_without_stringifying_it() {
        let mut long_path = PathBuf::from(r"C:\");
        for index in 0..10 {
            long_path.push(format!("segmento-não-ascii-{index:02}-complementar"));
        }
        long_path.push("Álbum.myalbuns");
        assert!(
            long_path.as_os_str().len() > 260,
            "the fixture must cross the legacy MAX_PATH boundary"
        );

        let cases = [
            (
                PathBuf::from(r"C:\Álbuns\João\Projeto.myalbuns"),
                PathBuf::from(r"C:\"),
            ),
            (long_path, PathBuf::from(r"C:\")),
            (
                PathBuf::from(r"\\servidor\acervo\Álbuns\Projeto.myalbuns"),
                PathBuf::from(r"\\servidor\acervo\"),
            ),
            (
                PathBuf::from(r"R:\Álbuns\Projeto.myalbuns"),
                PathBuf::from(r"\\servidor\acervo\"),
            ),
            (
                PathBuf::from(r"\\?\C:\Álbuns\Projeto.myalbuns"),
                PathBuf::from(r"\\?\C:\"),
            ),
            (
                PathBuf::from(r"\\?\UNC\servidor\acervo\Álbuns\Projeto.myalbuns"),
                PathBuf::from(r"\\?\UNC\servidor\acervo\"),
            ),
        ];

        for (logical_target, operational_root) in cases {
            let mut paths = OperationPathContext::new();
            paths
                .capture_with_binding(&logical_target, &operational_root)
                .unwrap_or_else(|error| {
                    panic!("{logical_target:?} must have a valid frozen binding: {error}")
                });
            let request = BootstrapRequest {
                protocol_version: PROTOCOL_VERSION,
                attempt_id: "attempt-path-matrix".into(),
                launch_nonce: "nonce-path-matrix".into(),
                intent: BootstrapIntent::OpenExisting,
                authority: TargetAuthority {
                    logical_target: NativePathDto::from(logical_target.clone()),
                    root_bindings: paths.freeze(),
                },
            };

            let encoded = serde_json::to_value(&request).expect("the request serializes");
            assert!(
                encoded["authority"]["logicalTarget"].is_object(),
                "the pathname itself stays a native DTO"
            );
            assert!(
                encoded["authority"]["rootBindings"]["bindings"]
                    .as_array()
                    .expect("bindings are serialized")
                    .iter()
                    .all(|binding| binding["logicalRoot"].is_object()
                        && binding["operationalRoot"].is_object()),
                "logical and operational roots also stay native DTOs"
            );
            let wire = serde_json::to_string(&encoded).expect("the JSON value encodes");
            assert!(!wire.contains("Álbuns"));
            assert!(!wire.contains("servidor"));

            let decoded: BootstrapRequest =
                serde_json::from_value(encoded).expect("the request deserializes");
            assert_eq!(decoded, request);
            assert_eq!(decoded.authority.logical_target.as_path(), logical_target);
            assert!(decoded.authority.root_bindings.covers(&logical_target));
        }
    }

    #[test]
    fn only_a_fully_correlated_ready_terminal_is_accepted() {
        let request = request();
        let terminal = HostTerminal::Ready {
            attempt_id: request.attempt_id.clone(),
            launch_nonce: request.launch_nonce.clone(),
            host_pid: 4312,
            project_id: "c4495826-fdf6-43ac-bbf9-92f068e6a704".into(),
            revision: 7,
        };

        assert_eq!(
            validate_terminal(&request, 4312, terminal),
            Ok(ValidatedTerminal::Ready {
                host_pid: 4312,
                project_id: "c4495826-fdf6-43ac-bbf9-92f068e6a704".into(),
                revision: 7,
            })
        );
    }

    #[test]
    fn terminal_validation_rejects_attempt_nonce_and_pid_mismatches() {
        let request = request();
        let terminals = [
            HostTerminal::Ready {
                attempt_id: "other-attempt".into(),
                launch_nonce: request.launch_nonce.clone(),
                host_pid: 4312,
                project_id: "project".into(),
                revision: 0,
            },
            HostTerminal::Ready {
                attempt_id: request.attempt_id.clone(),
                launch_nonce: "other-nonce".into(),
                host_pid: 4312,
                project_id: "project".into(),
                revision: 0,
            },
            HostTerminal::Ready {
                attempt_id: request.attempt_id.clone(),
                launch_nonce: request.launch_nonce.clone(),
                host_pid: 9999,
                project_id: "project".into(),
                revision: 0,
            },
        ];

        for terminal in terminals {
            assert_eq!(
                validate_terminal(&request, 4312, terminal),
                Err(TerminalValidationError::CorrelationMismatch)
            );
        }
    }

    #[test]
    fn a_correlated_failure_remains_structured() {
        let request = request();
        let terminal = HostTerminal::Failed {
            attempt_id: request.attempt_id.clone(),
            launch_nonce: request.launch_nonce.clone(),
            host_pid: 4312,
            stage: FailureStage::Open,
            code: FailureCode::ProjectInUse,
        };

        assert_eq!(
            validate_terminal(&request, 4312, terminal),
            Ok(ValidatedTerminal::Failed {
                host_pid: 4312,
                stage: FailureStage::Open,
                code: FailureCode::ProjectInUse,
            })
        );
    }

    #[test]
    fn one_shot_transport_reads_one_bounded_request_and_writes_one_terminal_line() {
        let request = request();
        let mut request_bytes = serde_json::to_vec(&request).expect("request serializes");
        request_bytes.push(b'\n');
        let decoded =
            read_bootstrap_request(request_bytes.as_slice()).expect("one request is accepted");
        assert_eq!(decoded, request);

        let terminal = HostTerminal::Ready {
            attempt_id: request.attempt_id,
            launch_nonce: request.launch_nonce,
            host_pid: 42,
            project_id: "project".into(),
            revision: 0,
        };
        let mut output = Vec::new();
        write_host_terminal(&mut output, &terminal).expect("one terminal is written");
        assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
        assert_eq!(
            serde_json::from_slice::<HostTerminal>(&output[..output.len() - 1])
                .expect("the terminal line is structured"),
            terminal
        );
    }

    #[test]
    fn one_shot_request_transport_rejects_trailing_values_and_oversized_input() {
        let request = serde_json::to_vec(&request()).expect("request serializes");
        let mut trailing = request.clone();
        trailing.extend_from_slice(b"\n{}");
        assert!(read_bootstrap_request(trailing.as_slice()).is_err());

        let oversized = vec![b' '; MAX_BOOTSTRAP_REQUEST_BYTES as usize + 1];
        assert!(read_bootstrap_request(oversized.as_slice()).is_err());
    }
}
