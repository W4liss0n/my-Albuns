mod cache;
mod render;
mod source;

use std::{
    io::{BufRead, Write},
    process::ExitCode,
};

use myalbuns_imaging_protocol::{
    IMAGING_PROTOCOL_VERSION, ImagingCommand, ImagingEvent, ImagingFailureStage, ImagingProgress,
    ImagingProgressStage, ImagingRequest, ImagingResponse, decode_command, encode_event,
};
use myalbuns_logging::{
    ProcessRole, init_local_logging, safe_log_identifier, sidecar_log_directory,
};
use myalbuns_paths::AppPaths;

use render::RenderFailure;

struct ProcessFailure {
    stage: Option<ImagingFailureStage>,
}

impl From<String> for ProcessFailure {
    fn from(_: String) -> Self {
        Self { stage: None }
    }
}

impl From<RenderFailure> for ProcessFailure {
    fn from(failure: RenderFailure) -> Self {
        Self {
            stage: Some(failure.stage),
        }
    }
}

fn main() -> ExitCode {
    let process_role = ProcessRole::Imaging;
    let app_paths = match AppPaths::discover() {
        Ok(app_paths) => app_paths,
        Err(error) => {
            eprintln!("pastas de dados do aplicativo indisponíveis: {error}");
            return ExitCode::FAILURE;
        }
    };
    let log_directory = sidecar_log_directory(&app_paths);
    let logging_guard = match init_local_logging(&log_directory, process_role) {
        Ok(guard) => Some(guard),
        Err(error) => {
            eprintln!("logging indisponível: {error}");
            None
        }
    };
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = process_role.as_str(),
        process_id = std::process::id(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        event = "imaging_process_started",
    );
    let exit_code = if let Err(failure) = run(&app_paths) {
        let stage = failure
            .stage
            .map_or("imaging_process", ImagingFailureStage::as_str);
        tracing::error!(
            target: "myalbuns.imaging",
            process_role = process_role.as_str(),
            protocol_version = IMAGING_PROTOCOL_VERSION,
            stage,
            event = "imaging_process_failed",
        );
        eprintln!("o Processador de Imagens não concluiu a solicitação.");
        failure
            .stage
            .map_or(ExitCode::FAILURE, |stage| ExitCode::from(stage.exit_code()))
    } else {
        tracing::info!(
            target: "myalbuns.imaging",
            process_role = process_role.as_str(),
            protocol_version = IMAGING_PROTOCOL_VERSION,
            event = "imaging_process_stopped",
            success = true,
        );
        ExitCode::SUCCESS
    };
    drop(logging_guard);
    exit_code
}

fn run(app_paths: &AppPaths) -> Result<(), ProcessFailure> {
    let mut source = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut source)
        .map_err(|error| {
            tracing::error!(
                target: "myalbuns.imaging",
                process_role = ProcessRole::Imaging.as_str(),
                protocol_version = IMAGING_PROTOCOL_VERSION,
                event = "imaging_request_read_failed",
            );
            format!("não foi possível ler a solicitação: {error}")
        })?;
    let command = decode_command(source.as_bytes()).inspect_err(|_| {
        tracing::error!(
            target: "myalbuns.imaging",
            process_role = ProcessRole::Imaging.as_str(),
            protocol_version = IMAGING_PROTOCOL_VERSION,
            event = "imaging_request_decode_failed",
        );
    })?;
    match command {
        ImagingCommand::Render(request) => run_render(request),
        ImagingCommand::BuildCache(request) => {
            cache::run_cache(request, app_paths).map_err(cache_failure)
        }
        ImagingCommand::ResetCache(request) => {
            cache::run_cache_reset(request, app_paths).map_err(cache_failure)
        }
    }
}

fn cache_failure(_: String) -> ProcessFailure {
    ProcessFailure {
        stage: Some(ImagingFailureStage::CacheProcessing),
    }
}

fn run_render(request: ImagingRequest) -> Result<(), ProcessFailure> {
    let operation_id = safe_log_identifier(&request.request_id);
    let project_id = safe_log_identifier(&request.snapshot.project_id);
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = ProcessRole::Imaging.as_str(),
        protocol_version = request.protocol_version,
        operation_id,
        project_id,
        process_id = std::process::id(),
        event = "imaging_request_started",
    );

    request.validate().inspect_err(|_| {
        tracing::warn!(
            target: "myalbuns.imaging",
            process_role = ProcessRole::Imaging.as_str(),
            protocol_version = request.protocol_version,
            operation_id,
            project_id,
            event = "imaging_request_rejected",
        );
    })?;

    let mut report_progress =
        |stage: ImagingProgressStage, completed_units: u32, total_units: u32| {
            write_progress(&request.request_id, stage, completed_units, total_units)
        };
    let completion =
        render::render_request(&request, &mut report_progress).inspect_err(|failure| {
            tracing::error!(
                target: "myalbuns.imaging",
                process_role = ProcessRole::Imaging.as_str(),
                protocol_version = request.protocol_version,
                operation_id,
                project_id,
                stage = failure.stage.as_str(),
                event = "imaging_render_failed",
            );
        })?;
    let response = ImagingResponse::completed(request.request_id.clone(), completion.clone());
    write_response(&response).inspect_err(|_| {
        tracing::error!(
            target: "myalbuns.imaging",
            process_role = ProcessRole::Imaging.as_str(),
            protocol_version = request.protocol_version,
            operation_id,
            project_id,
            event = "imaging_response_write_failed",
        );
    })?;
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = ProcessRole::Imaging.as_str(),
        protocol_version = request.protocol_version,
        operation_id,
        project_id,
        event = "imaging_request_completed",
        width_px = completion.width_px,
        height_px = completion.height_px,
        dpi = completion.dpi,
        source_count = completion.source_count,
        source_bytes = completion.source_bytes,
        output_bytes = completion.output_bytes,
        output_sha256 = completion.output_sha256.as_str(),
    );
    Ok(())
}

pub(crate) fn write_response(response: &ImagingResponse) -> Result<(), String> {
    write_event(&ImagingEvent::Response(response.clone()))
}

fn write_progress(
    request_id: &str,
    stage: ImagingProgressStage,
    completed_units: u32,
    total_units: u32,
) -> Result<(), String> {
    write_event(&ImagingEvent::Progress(ImagingProgress::new(
        request_id,
        stage,
        completed_units,
        total_units,
    )?))
}

fn write_event(event: &ImagingEvent) -> Result<(), String> {
    let encoded = encode_event(event)?;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&encoded)
        .and_then(|_| stdout.flush())
        .map_err(|error| format!("não foi possível responder: {error}"))
}
