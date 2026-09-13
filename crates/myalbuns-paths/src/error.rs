use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppPathsError {
    KnownFoldersUnavailable,
    InvalidProjectNamespace,
    InvalidStateNamespace,
    InvalidCacheArtifact,
    InvalidExportPath,
    InvalidOperationPath,
    UnsupportedOperationNamespace,
    PathRootNotBound,
    OperationPathAccessDenied,
    OperationPathUnavailable,
    OperationPathIoFailure,
    CacheArtifactOutsideRoot,
    CacheStorageUnavailable,
    CacheStorageFull,
    CacheStorageOutsideRoot,
    ExportStorageUnavailable,
    ExportStorageOutsideDestination,
    ExportTargetConflict,
}

impl AppPathsError {
    /// Retains actionable storage exhaustion without depending on localized OS text.
    pub fn cache_io(error: &std::io::Error) -> Self {
        #[cfg(windows)]
        if matches!(error.raw_os_error(), Some(39 | 112)) {
            return Self::CacheStorageFull;
        }
        match error.kind() {
            std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded => {
                Self::CacheStorageFull
            }
            _ => Self::CacheStorageUnavailable,
        }
    }
}

impl Display for AppPathsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::KnownFoldersUnavailable => {
                formatter.write_str("não foi possível localizar as pastas de dados do usuário")
            }
            Self::InvalidProjectNamespace => {
                formatter.write_str("a Identidade do Projeto não forma um namespace seguro")
            }
            Self::InvalidStateNamespace => {
                formatter.write_str("a identidade do estado local não forma um namespace seguro")
            }
            Self::InvalidCacheArtifact => {
                formatter.write_str("a identidade do artefato de Cache é inválida")
            }
            Self::InvalidExportPath => formatter.write_str("o caminho da Exportação é inválido"),
            Self::InvalidOperationPath => {
                formatter.write_str("o caminho externo da operação é inválido")
            }
            Self::UnsupportedOperationNamespace => {
                formatter.write_str("o namespace do caminho da operação não é aceito")
            }
            Self::PathRootNotBound => {
                formatter.write_str("a raiz do caminho não pertence ao plano da operação")
            }
            Self::OperationPathAccessDenied => {
                formatter.write_str("o acesso ao caminho da operação foi negado")
            }
            Self::OperationPathUnavailable => {
                formatter.write_str("o caminho da operação está temporariamente indisponível")
            }
            Self::OperationPathIoFailure => {
                formatter.write_str("a resolução do caminho da operação falhou")
            }
            Self::CacheArtifactOutsideRoot => {
                formatter.write_str("o artefato não pertence à raiz autorizada do Cache")
            }
            Self::CacheStorageUnavailable => {
                formatter.write_str("a estrutura de diretórios do Cache está indisponível")
            }
            Self::CacheStorageFull => formatter.write_str(
                "Não há espaço para preparar as imagens. Libere espaço no disco e abra o álbum novamente.",
            ),
            Self::CacheStorageOutsideRoot => {
                formatter.write_str("a estrutura física do Cache escapou da raiz autorizada")
            }
            Self::ExportStorageUnavailable => {
                formatter.write_str("a preparação da Exportação está indisponível")
            }
            Self::ExportStorageOutsideDestination => {
                formatter.write_str("a preparação da Exportação escapou do Destino autorizado")
            }
            Self::ExportTargetConflict => formatter.write_str(
                "o Destino da Exportação passou a existir sem autorização para substituição",
            ),
        }
    }
}

impl Error for AppPathsError {}
