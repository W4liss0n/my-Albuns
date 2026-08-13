use std::{
    io::{BufReader, Read},
    time::{SystemTime, UNIX_EPOCH},
};

use myalbuns_imaging_protocol::CacheFingerprint;
use myalbuns_paths::{ExpectedObject, ResolvedObject, RootBindingPlan};
use sha2::{Digest, Sha256};

pub(crate) fn fingerprint_source(
    media_id: &str,
    resolved: &ResolvedObject,
) -> Result<CacheFingerprint, String> {
    let file = resolved.reopen_for_read().map_err(|error| {
        format!("não foi possível abrir a mídia {media_id} no Processador: {error}")
    })?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("não foi possível inspecionar a mídia {media_id}: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("a mídia {media_id} não é um arquivo regular"));
    }
    let source_created_unix_ms = file_time_millis(metadata.created());
    let source_modified_unix_ms = file_time_millis(metadata.modified());
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut observed_bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("não foi possível verificar a mídia {media_id}: {error}"))?;
        if read == 0 {
            break;
        }
        observed_bytes = observed_bytes
            .checked_add(read as u64)
            .ok_or_else(|| format!("o tamanho da mídia {media_id} excedeu o limite"))?;
        hasher.update(&buffer[..read]);
    }
    let final_metadata = reader
        .get_ref()
        .metadata()
        .map_err(|error| format!("não foi possível reinspecionar a mídia {media_id}: {error}"))?;
    if observed_bytes != metadata.len()
        || final_metadata.len() != metadata.len()
        || file_time_millis(final_metadata.created()) != source_created_unix_ms
        || file_time_millis(final_metadata.modified()) != source_modified_unix_ms
    {
        return Err(format!(
            "a mídia {media_id} mudou durante a leitura do Processador"
        ));
    }
    CacheFingerprint::sha256_full_file_with_timestamps(
        observed_bytes,
        source_created_unix_ms,
        source_modified_unix_ms,
        format!("{:x}", hasher.finalize()),
    )
}

pub(crate) fn verify_source_fingerprint(
    media_id: &str,
    root_bindings: &RootBindingPlan,
    source_path: &std::path::Path,
    expected: &CacheFingerprint,
) -> Result<(), String> {
    let resolved = root_bindings
        .resolve_existing(source_path, ExpectedObject::RegularFile)
        .map_err(|error| {
            format!("não foi possível reabrir a mídia {media_id} pelo plano da operação: {error}")
        })?;
    let current = fingerprint_source(media_id, &resolved)?;
    if &current != expected {
        return Err(format!(
            "a mídia {media_id} mudou durante a produção da representação reduzida"
        ));
    }
    Ok(())
}

fn file_time_millis(time: std::io::Result<SystemTime>) -> Option<u64> {
    time.ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}
