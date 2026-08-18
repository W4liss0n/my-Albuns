use myalbuns_paths::ProcessInstanceId;
use serde::{Deserialize, Serialize};

use crate::is_safe_identifier;

const PROCESSOR_HANDSHAKE_SCHEMA_VERSION: u32 = 1;
pub const PROCESSOR_HANDSHAKE_CHALLENGE_ENV: &str = "MYALBUNS_PROCESSOR_HANDSHAKE_CHALLENGE";
pub const PROCESSOR_HANDSHAKE_MAX_BYTES: usize = 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProcessorHandshake<'a> {
    schema_version: u32,
    challenge: &'a str,
    process: ProcessInstanceId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OwnedProcessorHandshake {
    schema_version: u32,
    challenge: String,
    process: ProcessInstanceId,
}

pub fn encode_processor_handshake(
    challenge: &str,
    process: ProcessInstanceId,
) -> Result<Vec<u8>, String> {
    validate_challenge(challenge)?;
    let mut encoded = serde_json::to_vec(&ProcessorHandshake {
        schema_version: PROCESSOR_HANDSHAKE_SCHEMA_VERSION,
        challenge,
        process,
    })
    .map_err(|error| format!("could not encode the Processor handshake: {error}"))?;
    encoded.push(b'\n');
    if encoded.len() > PROCESSOR_HANDSHAKE_MAX_BYTES {
        return Err("the Processor handshake exceeds its size limit".into());
    }
    Ok(encoded)
}

pub fn decode_processor_handshake(
    source: &[u8],
    expected_challenge: &str,
    expected_process_id: u32,
) -> Result<ProcessInstanceId, String> {
    validate_challenge(expected_challenge)?;
    if source.is_empty()
        || source.len() > PROCESSOR_HANDSHAKE_MAX_BYTES
        || source.last() != Some(&b'\n')
        || source[..source.len() - 1].contains(&b'\n')
    {
        return Err("the Processor handshake must be one bounded JSON line".into());
    }
    let handshake: OwnedProcessorHandshake = serde_json::from_slice(&source[..source.len() - 1])
        .map_err(|error| format!("could not decode the Processor handshake: {error}"))?;
    if handshake.schema_version != PROCESSOR_HANDSHAKE_SCHEMA_VERSION {
        return Err("the Processor handshake schema is incompatible".into());
    }
    if handshake.challenge != expected_challenge {
        return Err("the Processor handshake challenge does not match this launch".into());
    }
    if handshake.process.process_id() != expected_process_id {
        return Err("the Processor handshake PID does not match the spawned child".into());
    }
    Ok(handshake.process)
}

fn validate_challenge(challenge: &str) -> Result<(), String> {
    if !is_safe_identifier(challenge) {
        return Err("the Processor handshake challenge is invalid".into());
    }
    Ok(())
}
