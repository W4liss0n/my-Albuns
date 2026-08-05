use serde::{Deserialize, Serialize};

use crate::is_safe_identifier;
use crate::response::ImagingResponse;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImagingProgressStage {
    LoadingSources,
    Composing,
    EncodingOutput,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagingProgress {
    pub request_id: String,
    pub stage: ImagingProgressStage,
    pub completed_units: u32,
    pub total_units: u32,
}

impl ImagingProgress {
    pub fn new(
        request_id: impl Into<String>,
        stage: ImagingProgressStage,
        completed_units: u32,
        total_units: u32,
    ) -> Result<Self, String> {
        let progress = Self {
            request_id: request_id.into(),
            stage,
            completed_units,
            total_units,
        };
        progress.validate()?;
        Ok(progress)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !is_safe_identifier(&self.request_id)
            || self.total_units == 0
            || self.completed_units > self.total_units
        {
            return Err("o progresso do Processador de Imagens é inválido".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "camelCase")]
pub enum ImagingEvent {
    Progress(ImagingProgress),
    Response(ImagingResponse),
}

pub fn encode_event(event: &ImagingEvent) -> Result<Vec<u8>, String> {
    let mut payload = serde_json::to_vec(event)
        .map_err(|error| format!("não foi possível serializar o evento: {error}"))?;
    payload.push(b'\n');
    Ok(payload)
}

pub fn decode_event(payload: &[u8]) -> Result<ImagingEvent, String> {
    let event: ImagingEvent = serde_json::from_slice(payload)
        .map_err(|error| format!("evento do Processador de Imagens inválido: {error}"))?;
    match &event {
        ImagingEvent::Progress(progress) => progress.validate()?,
        ImagingEvent::Response(response) if !is_safe_identifier(response.request_id()) => {
            return Err("a resposta final do Processador de Imagens é inválida".into());
        }
        ImagingEvent::Response(_) => {}
    }
    Ok(event)
}

const MAX_INCOMPLETE_EVENT_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct ImagingEventStreamDecoder {
    request_id: Option<String>,
    pending: Vec<u8>,
    progress: Option<ProgressCursor>,
    response: Option<ImagingResponse>,
}

#[derive(Clone, Copy, Debug)]
struct ProgressCursor {
    stage: ImagingProgressStage,
    completed_units: u32,
    total_units: u32,
}

impl ImagingProgressStage {
    const fn next(self) -> Option<Self> {
        match self {
            Self::LoadingSources => Some(Self::Composing),
            Self::Composing => Some(Self::EncodingOutput),
            Self::EncodingOutput => None,
        }
    }
}

impl ImagingEventStreamDecoder {
    pub fn new() -> Self {
        Self {
            request_id: None,
            pending: Vec::new(),
            progress: None,
            response: None,
        }
    }

    pub fn for_request(request_id: impl Into<String>) -> Result<Self, String> {
        let request_id = request_id.into();
        if !is_safe_identifier(&request_id) {
            return Err("a correlação do stream de eventos é inválida".into());
        }
        Ok(Self {
            request_id: Some(request_id),
            ..Self::new()
        })
    }

    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<ImagingProgress>, String> {
        self.pending.extend_from_slice(chunk);
        let mut emitted = Vec::new();
        while let Some(newline) = self.pending.iter().position(|byte| *byte == b'\n') {
            let remainder = self.pending.split_off(newline + 1);
            self.pending.truncate(newline);
            let line = std::mem::replace(&mut self.pending, remainder);
            if line.is_empty() {
                continue;
            }
            if let Some(progress) = self.accept(decode_event(&line)?)? {
                emitted.push(progress);
            }
        }
        if self.pending.len() > MAX_INCOMPLETE_EVENT_BYTES {
            return Err("o evento do Processador de Imagens excedeu o limite".into());
        }
        Ok(emitted)
    }

    pub fn finish(mut self) -> Result<ImagingResponse, String> {
        if !self.pending.is_empty() {
            return Err("o Processador devolveu um evento final incompleto".into());
        }
        self.response
            .take()
            .ok_or_else(|| "o Processador não devolveu uma resposta final".to_string())
    }

    fn accept(&mut self, event: ImagingEvent) -> Result<Option<ImagingProgress>, String> {
        match event {
            ImagingEvent::Progress(_) if self.response.is_some() => {
                Err("o Processador devolveu progresso após a resposta final".into())
            }
            ImagingEvent::Progress(progress) => {
                self.correlate(&progress.request_id)?;
                self.advance_progress(&progress)?;
                Ok(Some(progress))
            }
            ImagingEvent::Response(_) if self.response.is_some() => {
                Err("o Processador devolveu mais de uma resposta final".into())
            }
            ImagingEvent::Response(response) => {
                self.correlate(response.request_id())?;
                if !response.is_failure()
                    && let Some(progress) = self.progress
                    && (progress.stage != ImagingProgressStage::EncodingOutput
                        || progress.completed_units != progress.total_units)
                {
                    return Err("a resposta final chegou antes da conclusão do progresso".into());
                }
                self.response = Some(response);
                Ok(None)
            }
        }
    }

    fn correlate(&mut self, request_id: &str) -> Result<(), String> {
        match &self.request_id {
            Some(expected) if expected != request_id => {
                Err("o evento não corresponde à operação solicitada".into())
            }
            Some(_) => Ok(()),
            None => {
                self.request_id = Some(request_id.to_owned());
                Ok(())
            }
        }
    }

    fn advance_progress(&mut self, progress: &ImagingProgress) -> Result<(), String> {
        match self.progress {
            None if progress.stage != ImagingProgressStage::LoadingSources => {
                return Err("o progresso não começou pelo carregamento das fontes".into());
            }
            None => {}
            Some(previous) if progress.stage == previous.stage => {
                if progress.total_units != previous.total_units
                    || progress.completed_units < previous.completed_units
                {
                    return Err("o progresso do Processador de Imagens regrediu".into());
                }
            }
            Some(previous)
                if previous.stage.next() == Some(progress.stage)
                    && previous.completed_units == previous.total_units => {}
            Some(_) => {
                return Err("a ordem do progresso do Processador de Imagens é inválida".into());
            }
        }
        self.progress = Some(ProgressCursor {
            stage: progress.stage,
            completed_units: progress.completed_units,
            total_units: progress.total_units,
        });
        Ok(())
    }
}

impl Default for ImagingEventStreamDecoder {
    fn default() -> Self {
        Self::new()
    }
}

pub fn decode_event_stream(
    payload: &[u8],
) -> Result<(Vec<ImagingProgress>, ImagingResponse), String> {
    let mut decoder = ImagingEventStreamDecoder::new();
    let progress = decoder.push(payload)?;
    let response = decoder.finish()?;
    Ok((progress, response))
}
