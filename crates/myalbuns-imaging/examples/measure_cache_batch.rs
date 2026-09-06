//! Compare two native processors with fresh Cache namespaces and two workers.
//! Usage: measure_cache_batch inputs.json before.exe after.exe output-directory
use myalbuns_core::MediaKind;
use myalbuns_imaging_protocol::{
    CacheJob, CacheMediaSource, CacheRepresentationPolicy, CacheRequest, ImagingCommand,
    decode_event_stream,
};
use myalbuns_paths::{AppPaths, OperationPathContext, project_data_namespace};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

type Failure = Box<dyn std::error::Error + Send + Sync>;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

fn main() -> Result<(), Failure> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 4 {
        return Err("expected inputs.json before.exe after.exe output-directory".into());
    }
    let inputs: Vec<PathBuf> = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    if inputs.is_empty() {
        return Err("the input batch must not be empty".into());
    }
    let source_hashes = inputs
        .iter()
        .map(|path| digest(path))
        .collect::<Result<Vec<_>, _>>()?;
    let output = PathBuf::from(&args[3]);
    std::fs::create_dir_all(&output)?;
    let output = output.canonicalize()?;
    let mut measurements = Vec::new();
    for round in 0..7 {
        // Alternate order to reduce systematic disk-cache and temperature bias.
        for index in if round % 2 == 0 { [1, 2] } else { [2, 1] } {
            let label = if index == 1 { "before" } else { "after" };
            let run_directory = output.join(format!("{label}-{round}"));
            let measurement = measure(&inputs, Path::new(&args[index]), &run_directory)?;
            let entry =
                serde_json::json!({"variant": label, "round": round, "measurement": measurement});
            println!("{entry}");
            measurements.push(entry);
        }
    }
    let final_hashes = inputs
        .iter()
        .map(|path| digest(path))
        .collect::<Result<Vec<_>, _>>()?;
    if final_hashes != source_hashes {
        return Err("an original changed during measurement".into());
    }
    let report = serde_json::json!({
        "workers": 2, "files": inputs.len(), "originalsUnchanged": true,
        "sourceSha256": source_hashes, "measurements": measurements,
        "scope": "native Cache generation including process startup; excludes import validation, host publication and UI"
    });
    std::fs::write(
        output.join("measurements.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}

fn measure(
    inputs: &[PathBuf],
    processor: &Path,
    output: &Path,
) -> Result<serde_json::Value, Failure> {
    let runtime = output.join("runtime");
    std::fs::create_dir_all(runtime.join("Local"))?;
    std::fs::create_dir_all(runtime.join("Roaming"))?;
    let logs = output.join("logs");
    std::fs::create_dir_all(&logs)?;
    let app_paths = AppPaths::from_roots(&runtime.join("Roaming"), &runtime.join("Local"));
    let project_id = format!("cache-measurement-{}", std::process::id());
    let cache = app_paths.project_cache(&project_data_namespace(&project_id))?;
    let jobs = inputs
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let media_id = format!("photo-{index}");
            let request_id = format!("request-{index}");
            let mut context = OperationPathContext::new();
            context.capture(path)?;
            context.capture(cache.root())?;
            let source = CacheMediaSource::new(&media_id, MediaKind::Photo, path.clone())?;
            let command = ImagingCommand::build_cache(CacheRequest::new(
                &request_id,
                &project_id,
                cache.clone(),
                vec![CacheJob::new(source, "generation-1", None)?],
                CacheRepresentationPolicy::measured_v1(),
                context.freeze(),
            )?);
            Ok((request_id, serde_json::to_vec(&command)?))
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let next = AtomicUsize::new(0);
    let started = Instant::now();
    let results = std::thread::scope(|scope| {
        let workers = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    let mut completed = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some((_, payload)) = jobs.get(index) else {
                            break;
                        };
                        completed.push(
                            invoke(processor, &runtime, &logs, payload)
                                .map(|result| (index, result)),
                        );
                    }
                    completed
                })
            })
            .collect::<Vec<_>>();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("benchmark worker panicked"))
            .collect::<Vec<_>>()
    });
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut artifacts = Vec::new();
    for result in results {
        let (index, result) = result?;
        if !result.status.success() {
            return Err(format!(
                "processor failed: {}",
                String::from_utf8_lossy(&result.stderr)
            )
            .into());
        }
        let (_, response) = decode_event_stream(&result.stdout)?;
        let completed = response
            .cache_completed_for(&jobs[index].0)
            .ok_or("missing completion")?;
        let artifact = completed.artifacts.first().ok_or("missing artifact")?;
        let preview =
            cache.preview_file(&artifact.media_id, &artifact.generation_id, artifact.format)?;
        let decoded = image::open(&preview)?;
        artifacts.push((
            index,
            serde_json::json!({
                "sha256": digest(&preview)?, "width": decoded.width(), "height": decoded.height(),
                "bytes": artifact.preview_bytes
            }),
        ));
    }
    artifacts.sort_by_key(|(index, _)| *index);
    Ok(
        serde_json::json!({"elapsedMs": elapsed_ms, "artifacts": artifacts.into_iter().map(|(_, artifact)| artifact).collect::<Vec<_>>()}),
    )
}

fn invoke(
    processor: &Path,
    runtime: &Path,
    logs: &Path,
    payload: &[u8],
) -> Result<Output, Failure> {
    let mut command = Command::new(processor);
    #[cfg(windows)]
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    let mut child = command
        .env("MYALBUNS_PROCESS_GATE_DATA_ROOT", runtime)
        .env("MYALBUNS_LOG_DIR", logs)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().ok_or("missing processor stdin")?;
    stdin.write_all(payload)?;
    stdin.write_all(b"\n")?;
    drop(stdin);
    Ok(child.wait_with_output()?)
}

fn digest(path: &Path) -> Result<String, Failure> {
    Ok(format!("{:x}", Sha256::digest(std::fs::read(path)?)))
}
