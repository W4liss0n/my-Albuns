use std::{env, path::PathBuf};

use myalbuns_core::{EditorProjection, ExportResult, ProjectIntent};
use ts_rs::{Config, TS};

fn main() {
    let output_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("src/domain/generated"));
    // Tauri's JSON transport delivers these bounded domain values as
    // JavaScript numbers, so the generated contract must describe that wire.
    let config = Config::new()
        .with_out_dir(output_dir)
        .with_large_int("number");

    EditorProjection::export_all(&config).expect("EditorProjection bindings should be generated");
    ProjectIntent::export_all(&config).expect("ProjectIntent bindings should be generated");
    ExportResult::export_all(&config).expect("ExportResult bindings should be generated");
}
