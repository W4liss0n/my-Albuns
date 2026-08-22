#![cfg(windows)]

use std::{
    collections::HashMap,
    ffi::OsString,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
};

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, InitialProject, OpenProjectError,
    OpenProjectRequest, ProjectCore, ProjectIntent, ProjectLocation, SaveCopyAsError,
    SaveCopyAsRequest,
};
use myalbuns_paths::{OperationPathContext, project_data_namespace};
use serde_json::{Value, json};

fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let command = arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .expect("one UTF-8 gate command is required");
    let arguments = parse_arguments(arguments.collect());
    let core = ProjectCore::new().with_identity_storage_roots(
        required_path(&arguments, "--lease-root"),
        required_path(&arguments, "--registry-root"),
    );
    let project_path = required_path(&arguments, "--project");

    let result = match command.as_str() {
        "create" => create(&core, &project_path),
        "open" => open(&core, &project_path),
        "hold" => hold(
            &core,
            &project_path,
            optional_u32(&arguments, "--pending-dpi"),
        ),
        "edit-save" => edit_save(&core, &project_path, required_u32(&arguments, "--dpi")),
        "save-copy-as" => save_copy_as(
            &core,
            &project_path,
            &required_path(&arguments, "--destination"),
        ),
        other => panic!("unsupported gate command: {other}"),
    };

    println!(
        "{}",
        serde_json::to_string(&result).expect("gate result serializes")
    );
}

fn parse_arguments(arguments: Vec<OsString>) -> HashMap<String, OsString> {
    assert!(
        arguments.len().is_multiple_of(2),
        "gate arguments are explicit flag/value pairs"
    );
    arguments
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let flag = pair[0]
                .to_str()
                .filter(|value| value.starts_with("--"))
                .expect("gate flags are UTF-8 long options")
                .to_owned();
            (flag, pair[1].clone())
        })
        .collect()
}

fn required_path(arguments: &HashMap<String, OsString>, name: &str) -> PathBuf {
    arguments
        .get(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("missing required argument {name}"))
}

fn required_u32(arguments: &HashMap<String, OsString>, name: &str) -> u32 {
    optional_u32(arguments, name).unwrap_or_else(|| panic!("missing required argument {name}"))
}

fn optional_u32(arguments: &HashMap<String, OsString>, name: &str) -> Option<u32> {
    arguments.get(name).map(|value| {
        value
            .to_str()
            .and_then(|value| value.parse().ok())
            .filter(|value| *value > 0)
            .unwrap_or_else(|| panic!("{name} must be a positive u32"))
    })
}

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context
        .capture(path)
        .expect("the explicit gate pathname has a capturable Windows root");
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn create(core: &ProjectCore, project_path: &Path) -> Value {
    match core.create_editable(CreateProjectRequest::new(
        location(project_path),
        InitialProject::neutral(),
        CreateAuthorization::CreateOnly,
    )) {
        Ok(project) => opened_result(&project),
        Err(error) => json!({ "status": "createError", "error": format!("{error:?}") }),
    }
}

fn open(core: &ProjectCore, project_path: &Path) -> Value {
    match core.open_editable(OpenProjectRequest::new(location(project_path))) {
        Ok(project) => opened_result(&project),
        Err(error) => open_error(error),
    }
}

fn hold(core: &ProjectCore, project_path: &Path, pending_dpi: Option<u32>) -> Value {
    let mut project = match core.open_editable(OpenProjectRequest::new(location(project_path))) {
        Ok(project) => project,
        Err(error) => return open_error(error),
    };
    if let Some(dpi) = pending_dpi {
        project
            .apply(ProjectIntent::SetDpi { dpi })
            .expect("the pending creative change is valid");
    }
    let ready = json!({
        "status": "holding",
        "pid": std::process::id(),
        "projectId": project.project_id().hyphenated().to_string(),
        "revision": project.revision(),
        "savedRevision": project.saved_revision(),
        "dpi": project.projection().state.document.dpi,
        "dirty": project.has_unsaved_changes(),
        "namespace": project_data_namespace(&project.project_id().hyphenated().to_string()),
    });
    println!(
        "{}",
        serde_json::to_string(&ready).expect("ready result serializes")
    );
    io::stdout()
        .flush()
        .expect("the holder readiness is visible");
    let mut release = String::new();
    io::stdin()
        .lock()
        .read_line(&mut release)
        .expect("the holder receives its explicit release signal");
    assert_eq!(
        release.trim(),
        "release",
        "the holder release is correlated"
    );
    json!({ "status": "released" })
}

fn edit_save(core: &ProjectCore, project_path: &Path, dpi: u32) -> Value {
    let mut project = match core.open_editable(OpenProjectRequest::new(location(project_path))) {
        Ok(project) => project,
        Err(error) => return open_error(error),
    };
    if let Err(error) = project.apply(ProjectIntent::SetDpi { dpi }) {
        return json!({ "status": "editError", "error": format!("{error:?}") });
    }
    if let Err(error) = project.save(project.revision()) {
        return json!({ "status": "saveError", "error": format!("{error:?}") });
    }
    opened_result(&project)
}

fn save_copy_as(core: &ProjectCore, source_path: &Path, destination_path: &Path) -> Value {
    let source = match core.open_editable(OpenProjectRequest::new(location(source_path))) {
        Err(OpenProjectError::ExternalCopyNotWritable(source)) => *source,
        Err(error) => return open_error(error),
        Ok(_) => return json!({ "status": "sourceUnexpectedlyEditable" }),
    };
    match core.save_copy_as(SaveCopyAsRequest::new(
        source,
        location(destination_path),
        CreateAuthorization::CreateOnly,
    )) {
        Ok(project) => opened_result(&project),
        Err(error) => save_copy_error(error),
    }
}

fn opened_result(project: &myalbuns_core::EditableProject) -> Value {
    json!({
        "status": "opened",
        "projectId": project.project_id().hyphenated().to_string(),
        "revision": project.revision(),
        "savedRevision": project.saved_revision(),
        "dpi": project.projection().state.document.dpi,
        "dirty": project.has_unsaved_changes(),
        "canUndo": project.can_undo(),
        "namespace": project_data_namespace(&project.project_id().hyphenated().to_string()),
    })
}

fn open_error(error: OpenProjectError) -> Value {
    match error {
        OpenProjectError::FocusExisting {
            project_id,
            owner_process,
        } => json!({
            "status": "focusExisting",
            "projectId": project_id.hyphenated().to_string(),
            "ownerProcess": owner_process,
        }),
        OpenProjectError::ExternalCopyNotWritable(_) => {
            json!({ "status": "externalCopyNotWritable" })
        }
        OpenProjectError::IdentityIndeterminate => json!({ "status": "identityIndeterminate" }),
        OpenProjectError::ProjectInUse => json!({ "status": "projectInUse" }),
        OpenProjectError::ExternalCopyRequiresInteractiveResolution => {
            json!({ "status": "externalCopyRequiresInteractiveResolution" })
        }
        OpenProjectError::Path(error) => {
            json!({ "status": "pathError", "error": format!("{error:?}") })
        }
        OpenProjectError::Document(error) => {
            json!({ "status": "documentError", "error": format!("{error:?}") })
        }
    }
}

fn save_copy_error(error: SaveCopyAsError) -> Value {
    let status = match error {
        SaveCopyAsError::DestinationConflict => "destinationConflict",
        SaveCopyAsError::ProjectInUse => "projectInUse",
        SaveCopyAsError::IdentityIndeterminate => "identityIndeterminate",
        SaveCopyAsError::SaveCopyStateIndeterminate => "saveCopyStateIndeterminate",
        SaveCopyAsError::Path(_) => "pathError",
    };
    json!({ "status": status })
}
