use std::{env, path::PathBuf};

use myalbuns_desktop_lib::ipc_contract::{
    CancelDisposition, ExportCommandError, ExportEvent, ExportResult, FrontendLogEvent,
    MediaPreview,
};
use ts_rs::{Config, TS};

fn main() {
    let output_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("src/platform/generated"));
    let config = Config::new()
        .with_out_dir(output_dir)
        .with_large_int("number");

    CancelDisposition::export_all(&config)
        .expect("cancel disposition bindings should be generated");
    ExportCommandError::export_all(&config).expect("export error bindings should be generated");
    ExportEvent::export_all(&config).expect("export event bindings should be generated");
    ExportResult::export_all(&config).expect("export result bindings should be generated");
    FrontendLogEvent::export_all(&config).expect("frontend log bindings should be generated");
    MediaPreview::export_all(&config).expect("media preview bindings should be generated");
}
