use std::path::{Path, PathBuf};

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, InitialProject, ProjectCore, ProjectLocation,
};
use myalbuns_paths::{AppPaths, OperationPathContext};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FixtureManifest {
    original_path: PathBuf,
    external_copy_path: PathBuf,
    project_id: String,
    source_revision: u64,
}

fn project_location(path: &Path) -> Result<ProjectLocation, Box<dyn std::error::Error>> {
    let mut paths = OperationPathContext::new();
    paths.capture(path)?;
    Ok(ProjectLocation::new(path.to_path_buf(), paths.freeze()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixture_root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: prepare_focused_owned_dialog_fixtures <fixture-root>")?;
    if !fixture_root.is_absolute() {
        return Err("the fixture root must be absolute".into());
    }
    std::fs::create_dir_all(&fixture_root)?;
    let original_path = fixture_root.join("Projeto focado.myalbuns");
    let external_copy_path = fixture_root.join("Cópia externa focada.myalbuns");
    let manifest_path = fixture_root.join("focused-owned-dialog-fixture.json");
    if original_path.exists() || external_copy_path.exists() || manifest_path.exists() {
        return Err("the focused fixture requires absent targets".into());
    }

    let app_paths = AppPaths::discover()?;
    let core = ProjectCore::new().with_identity_storage_roots(
        app_paths.project_identity_leases_dir(),
        app_paths.project_identities_dir(),
    );
    let project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path)?,
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .map_err(|error| format!("the focused Project fixture could not be created: {error:?}"))?;
    let project_id = project.project_id().hyphenated().to_string();
    let source_revision = project.revision();
    drop(project);

    std::fs::copy(&original_path, &external_copy_path)?;
    let mut permissions = std::fs::metadata(&external_copy_path)?.permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&external_copy_path, permissions)?;

    let manifest = FixtureManifest {
        original_path,
        external_copy_path,
        project_id,
        source_revision,
    };
    let json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&manifest_path, format!("{json}\n"))?;
    println!("{}", serde_json::to_string(&manifest)?);
    Ok(())
}
