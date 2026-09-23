use std::{path::PathBuf, sync::Mutex};

use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent};

use crate::{
    cache_engine::CacheEngine,
    cache_previews::CachePreviewRegistry,
    cache_service::ActiveCacheNamespace,
    eye_correction::{self, Face, PreparedEyes},
    image_processing::ImageProcessingBatch,
    project_host::ProjectHost,
    media_runtime::MediaResolver,
    ipc_contract::{ViewerAction, ViewerPresentation},
    native_dialog_window,
    product_runtime::PROJECT_WINDOW_LABEL,
};
use myalbuns_paths::AppPaths;

#[cfg(debug_assertions)]
use crate::desktop_webview_policy;

pub(crate) const LABEL: &str = "image-viewer";
pub(crate) const NAVIGATE_EVENT: &str = "myalbuns://image-viewer-navigate";
pub(crate) const CLOSED_EVENT: &str = "myalbuns://image-viewer-closed";
pub(crate) const PRESENTATION_EVENT: &str = "myalbuns://image-viewer-presentation";

#[derive(Default)]
pub(crate) struct ViewerStore(Mutex<Option<ViewerPresentation>>);

impl ViewerStore {
    fn current(&self) -> Result<Option<ViewerPresentation>, String> {
        self.0
            .lock()
            .map(|value| value.clone())
            .map_err(|_| "viewer state unavailable".into())
    }

    fn replace(&self, presentation: ViewerPresentation) -> Result<(), String> {
        let mut value = self.0.lock().map_err(|_| "viewer state unavailable")?;
        if value
            .as_ref()
            .is_some_and(|old| old.session_id != presentation.session_id)
        {
            return Err("another viewer session is active".into());
        }
        if value
            .as_ref()
            .is_some_and(|old| old.revision > presentation.revision)
        {
            return Ok(());
        }
        *value = Some(presentation);
        Ok(())
    }

    fn clear(&self, session_id: &str) -> Result<bool, String> {
        let mut value = self.0.lock().map_err(|_| "viewer state unavailable")?;
        if value
            .as_ref()
            .is_some_and(|old| old.session_id == session_id)
        {
            *value = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn validate(
    presentation: &mut ViewerPresentation,
    previews: &CachePreviewRegistry,
) -> Result<(), String> {
    if presentation.session_id.len() > 128
        || presentation.session_id.is_empty()
        || presentation.media_id.len() > 256
        || presentation.media_id.is_empty()
    {
        return Err("invalid viewer presentation identity".into());
    }
    presentation.name = presentation.name.chars().take(512).collect();
    if presentation
        .url
        .as_deref()
        .is_some_and(|url| !previews.is_published_url(url))
    {
        return Err("the viewer URL is not a published cache preview".into());
    }
    if let Some(correction) = &presentation.correction {
        if correction.reference_url.as_deref().is_some_and(|url| !previews.is_published_url(url))
            || correction.result_url.as_deref().is_some_and(|url| !previews.is_published_url(url)) {
            return Err("a correction URL is not a published preview".into());
        }
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn open_image_viewer(
    app: AppHandle,
    window: WebviewWindow,
    mut presentation: ViewerPresentation,
    store: State<'_, ViewerStore>,
    previews: State<'_, CachePreviewRegistry>,
) -> Result<(), String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("only the Project window can open the viewer".into());
    }
    validate(&mut presentation, &previews)?;
    store.replace(presentation.clone())?;
    previews.set_viewer_access(true);
    let result = async {
        if let Some(existing) = app.get_webview_window(LABEL) {
            existing
                .emit(PRESENTATION_EVENT, &presentation)
                .map_err(|error| error.to_string())?;
            return native_dialog_window::display_owned_dialog(&window, &existing)
                .map_err(|error| error.to_string());
        }
        #[cfg(debug_assertions)]
        let browser_arguments = desktop_webview_policy::replacement_webview_debug_arguments(
            std::env::var_os("MYALBUNS_DEV_IMAGE_VIEWER_WEBVIEW_DEBUG_PORT"),
        )
        .map_err(|error| error.to_string())?;
        #[cfg(debug_assertions)]
        let browser_data_directory = desktop_webview_policy::project_dialog_debug_data_directory(
            std::env::var_os("MYALBUNS_DEV_IMAGE_VIEWER_WEBVIEW_DATA_DIRECTORY"),
        )
        .map_err(|error| error.to_string())?;
        #[cfg(not(debug_assertions))]
        let browser_arguments: Option<String> = None;
        #[cfg(not(debug_assertions))]
        let browser_data_directory: Option<std::path::PathBuf> = None;
        let viewer = native_dialog_window::build_hidden_owned_viewer_window(
            &app,
            &window,
            native_dialog_window::HiddenOwnedWindowConfig {
                label: LABEL,
                url: "image-viewer.html",
                width: 1050.0,
                height: 720.0,
                browser_arguments: browser_arguments.as_deref(),
                browser_data_directory: browser_data_directory.as_deref(),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        let owner = window.clone();
        let app_for_close = app.clone();
        let session_for_close = presentation.session_id.clone();
        viewer.on_window_event(move |event| {
            if matches!(event, WindowEvent::Destroyed) {
                retire(&app_for_close, &owner, &session_for_close);
            }
        });
        if store
            .current()?
            .as_ref()
            .is_none_or(|current| current.session_id != presentation.session_id)
        {
            let _ = native_dialog_window::dismiss_blocked_window(&window, &viewer, true);
            return Ok(());
        }
        native_dialog_window::display_owned_dialog(&window, &viewer)
            .map_err(|error| error.to_string())
    }
    .await;
    if result.is_err() {
        if let Some(viewer) = app.get_webview_window(LABEL) {
            let _ = native_dialog_window::dismiss_blocked_window(&window, &viewer, true);
        }
        retire(&app, &window, &presentation.session_id);
    }
    result
}

#[tauri::command]
pub(crate) fn update_image_viewer(
    window: WebviewWindow,
    mut presentation: ViewerPresentation,
    store: State<'_, ViewerStore>,
    previews: State<'_, CachePreviewRegistry>,
    app: AppHandle,
) -> Result<(), String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("only the Project window can update the viewer".into());
    }
    validate(&mut presentation, &previews)?;
    if store
        .current()?
        .as_ref()
        .is_none_or(|old| old.session_id != presentation.session_id)
    {
        return Ok(());
    }
    store.replace(presentation.clone())?;
    if let Some(viewer) = app.get_webview_window(LABEL) {
        viewer
            .emit(PRESENTATION_EVENT, presentation)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn current_image_viewer(
    window: WebviewWindow,
    store: State<'_, ViewerStore>,
) -> Result<Option<ViewerPresentation>, String> {
    if window.label() != LABEL {
        return Err("viewer presentation belongs to the viewer window".into());
    }
    store.current()
}

#[tauri::command]
pub(crate) fn navigate_image_viewer(
    app: AppHandle,
    window: WebviewWindow,
    session_id: String,
    offset: i8,
    store: State<'_, ViewerStore>,
) -> Result<(), String> {
    if window.label() != LABEL || !matches!(offset, -1 | 1) {
        return Err("invalid viewer navigation".into());
    }
    let Some(current) = store.current()? else {
        return Ok(());
    };
    if current.session_id != session_id
        || (offset == -1 && !current.correction.as_ref().map_or(current.can_previous, |correction| correction.can_previous_reference))
        || (offset == 1 && !current.correction.as_ref().map_or(current.can_next, |correction| correction.can_next_reference))
    {
        return Ok(());
    }
    app.emit_to(
        PROJECT_WINDOW_LABEL,
        NAVIGATE_EVENT,
        ViewerAction { session_id, offset },
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn close_image_viewer(
    app: AppHandle,
    window: WebviewWindow,
    session_id: String,
    store: State<'_, ViewerStore>,
) -> Result<(), String> {
    if window.label() != LABEL && window.label() != PROJECT_WINDOW_LABEL {
        return Err("invalid viewer closer".into());
    }
    if store
        .current()?
        .as_ref()
        .is_none_or(|current| current.session_id != session_id)
    {
        return Ok(());
    }
    let owner = app
        .get_webview_window(PROJECT_WINDOW_LABEL)
        .ok_or("the Project window is unavailable")?;
    if app.state::<CorrectionStore>().is_applying() {
        return Err("A correção está sendo aplicada.".into());
    }
    if let Some(viewer) = app.get_webview_window(LABEL) {
        native_dialog_window::dismiss_blocked_window(&owner, &viewer, true)
            .map_err(|error| error.to_string())?;
    }
    retire(&app, &owner, &session_id);
    Ok(())
}

#[derive(Clone)]
struct PendingEyes {
    session_id: String,
    media_id: String,
    token: String,
    path: PathBuf,
    url: String,
    original_path: PathBuf,
    original_digest: [u8; 32],
}

#[derive(Default)]
struct CorrectionState { generation: u64, pending: Option<PendingEyes>, applying: bool }

#[derive(Default)]
pub(crate) struct CorrectionStore(Mutex<CorrectionState>);

impl CorrectionStore {
    fn clear(&self, previews: &CachePreviewRegistry) -> Option<u64> {
        let mut state = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.applying { return None; }
        state.generation = state.generation.wrapping_add(1);
        if let Some(pending) = state.pending.take() {
            previews.revoke_viewer_preview(&pending.url);
            let _ = std::fs::remove_file(pending.path);
        }
        Some(state.generation)
    }

    fn begin(&self, previews: &CachePreviewRegistry) -> Result<u64, String> {
        self.clear(previews).ok_or_else(|| "A correção está sendo aplicada.".into())
    }

    fn accept(&self, generation: u64, pending: PendingEyes) -> bool {
        let mut state = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.generation != generation { return false; }
        state.pending = Some(pending);
        true
    }

    fn is_applying(&self) -> bool {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).applying
    }

    fn claim_for_apply(&self, token: &str, session_id: &str) -> Option<PendingEyes> {
        let mut state = self.0.lock().ok()?;
        if state.applying { return None; }
        let pending = state.pending.as_ref().filter(|pending|
            pending.token == token && pending.session_id == session_id
        )?.clone();
        state.applying = true;
        Some(pending)
    }

    fn release_apply(&self) {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).applying = false;
    }

    fn finish(&self) {
        let mut state = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.generation = state.generation.wrapping_add(1);
        state.pending = None;
        state.applying = false;
    }
}

#[tauri::command]
pub(crate) async fn prepare_eye_correction(
    window: WebviewWindow,
    session_id: String,
    target_media_id: String,
    reference_media_id: String,
    target_face: Face,
    reference_face: Face,
    viewer: State<'_, ViewerStore>,
    corrections: State<'_, CorrectionStore>,
    previews: State<'_, CachePreviewRegistry>,
    host: State<'_, ProjectHost>,
) -> Result<PreparedEyes, String> {
    if window.label() != PROJECT_WINDOW_LABEL { return Err("A correção só pode ser preparada pelo Projeto.".into()); }
    if viewer.current()?.as_ref().is_none_or(|current|
        current.session_id != session_id || current.media_id != target_media_id
    ) { return Err("Esta sessão do visualizador não está mais ativa.".into()); }
    let catalog = host.authorized_media_catalog()?;
    let target = catalog.bindings.iter().find(|binding| binding.media_id == target_media_id && binding.kind == myalbuns_core::MediaKind::Photo)
        .ok_or("A foto de destino não pertence ao projeto.")?;
    let reference = catalog.bindings.iter().find(|binding| binding.media_id == reference_media_id && binding.kind == myalbuns_core::MediaKind::Photo)
        .ok_or("A foto de referência não pertence ao projeto.")?;
    if target.media_id == reference.media_id { return Err("Escolha outra foto como referência.".into()); }
    let output = eye_correction::corrected_path(&host.project_directory()?, &target.logical_path)?;
    let target_path = target.logical_path.clone();
    let reference_path = reference.logical_path.clone();
    let generation = corrections.begin(&previews)?;
    let output_for_render = output.clone();
    let rendered = tauri::async_runtime::spawn_blocking(move || {
        let before = eye_correction::source_digest(&target_path)?;
        let bytes = eye_correction::render(&target_path, &reference_path, &target_face, &reference_face, &output_for_render)?;
        if eye_correction::source_digest(&target_path)? != before {
            return Err("A foto original mudou durante a preparação. Tente novamente.".into());
        }
        Ok((bytes, before))
    }).await.map_err(|_| "A correção foi interrompida.".to_string());
    let (bytes, original_digest) = match rendered {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => { let _ = std::fs::remove_file(&output); let _ = std::fs::remove_file(output.with_extension("tmp")); return Err(error); }
        Err(error) => { let _ = std::fs::remove_file(&output); let _ = std::fs::remove_file(output.with_extension("tmp")); return Err(error); }
    };
    let url = previews.publish_viewer_preview(bytes);
    let token = uuid::Uuid::new_v4().to_string();
    let pending = PendingEyes { session_id, media_id: target_media_id, token: token.clone(), path: output.clone(), url: url.clone(),
        original_path: target.logical_path.clone(), original_digest };
    if !corrections.accept(generation, pending) {
        previews.revoke_viewer_preview(&url);
        let _ = std::fs::remove_file(output);
        return Err("A correção foi cancelada.".into());
    }
    Ok(PreparedEyes { token, url })
}

#[tauri::command]
pub(crate) fn cancel_eye_correction(
    window: WebviewWindow,
    corrections: State<'_, CorrectionStore>,
    previews: State<'_, CachePreviewRegistry>,
) -> Result<(), String> {
    if window.label() != PROJECT_WINDOW_LABEL { return Err("A correção só pode ser cancelada pelo Projeto.".into()); }
    corrections.clear(&previews).ok_or("A correção está sendo aplicada.")?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn apply_eye_correction(
    app: AppHandle,
    window: WebviewWindow,
    session_id: String,
    token: String,
    viewer: State<'_, ViewerStore>,
    corrections: State<'_, CorrectionStore>,
    previews: State<'_, CachePreviewRegistry>,
    host: State<'_, ProjectHost>,
) -> Result<myalbuns_core::EditorProjection, String> {
    if window.label() != PROJECT_WINDOW_LABEL { return Err("A correção só pode ser aplicada pelo Projeto.".into()); }
    let pending = corrections.claim_for_apply(&token, &session_id).ok_or("A prévia da correção expirou.")?;
    let result = async {
    if viewer.current()?.as_ref().is_none_or(|current|
        current.session_id != session_id || current.media_id != pending.media_id
    ) { return Err("Esta sessão do visualizador não está mais ativa.".into()); }
    let _operation = crate::project_ui_operations::begin(&app)?;
    let catalog = host.authorized_media_catalog()?;
    let binding = catalog.bindings.iter().find(|binding| binding.media_id == pending.media_id)
        .ok_or("A foto não pertence mais ao projeto.")?.clone();
    if binding.logical_path != pending.original_path { return Err("A foto original mudou desde a prévia.".into()); }
    let affected: Vec<_> = catalog.bindings.iter().filter(|media| media.logical_path == pending.original_path).cloned().collect();
    let mut paths = myalbuns_paths::OperationPathContext::new();
    let cache_root = app.state::<crate::cache_service::ActiveCacheNamespace>().namespace().paths().root().to_path_buf();
    paths.capture(&cache_root).map_err(|error| error.to_string())?;
    for media in &catalog.bindings { paths.capture(&media.logical_path).map_err(|error| error.to_string())?; }
    paths.capture(&pending.path).map_err(|error| error.to_string())?;
    let roots = paths.freeze();
    let engine = app.state::<CacheEngine>();
    let pause = engine.pause().await;
    let active = app.state::<ActiveCacheNamespace>();
    let app_paths = app.state::<AppPaths>();
    for media in &affected {
        engine.invalidate_relinked_media(&pause, &app_paths, &active.namespace(), &previews, &media.media_id)
            .map_err(|error| error.message)?;
    }
    let original = pending.original_path.clone();
    let prepared = pending.path.clone();
    let digest = pending.original_digest;
    let replacement = tauri::async_runtime::spawn_blocking(move || eye_correction::replace_original(&original, &prepared, digest))
        .await.map_err(|_| "A substituição do original foi interrompida.".to_string()).and_then(|value| value);
    let backup = match replacement {
        Ok(backup) => backup,
        Err(error) => {
            drop(pause);
            let mut repair = ImageProcessingBatch::new(affected.len() as u32, |_| {});
            repair.prepare_all_in_plan(&app, affected, roots).await;
            return Err(error);
        }
    };
    let refreshed = (|| {
        for media in &affected {
            let metadata = MediaResolver.inspect_media_binding_in_plan(media, &roots)?;
            host.observe_photo_source(media, metadata)?;
        }
        host.projection()
    })();
    let projection = match refreshed {
        Ok(projection) => projection,
        Err(error) => {
            eye_correction::restore_original(&pending.original_path, &backup)?;
            for media in &affected {
                if let Ok(metadata) = MediaResolver.inspect_media_binding_in_plan(media, &roots) {
                    let _ = host.observe_photo_source(media, metadata);
                }
            }
            return Err(error);
        }
    };
    drop(pause);
    let mut cache_failure = None;
    let mut completed = 0;
    let mut progress = ImageProcessingBatch::new(affected.len() as u32, |event| {
        completed = event.completed_files;
        if let Some(problem) = event.problem { cache_failure = Some(problem.reason); }
        if let Some(problem) = event.operation_problem { cache_failure = Some(problem); }
    });
    progress.prepare_all_in_plan(&app, affected.clone(), roots.clone()).await;
    drop(progress);
    if cache_failure.is_none() && completed < affected.len() as u32 {
        cache_failure = Some("A atualização da prévia temporária não foi concluída.".into());
    }
    if let Some(error) = cache_failure {
        eye_correction::restore_original(&pending.original_path, &backup)?;
        for media in &affected {
            let metadata = MediaResolver.inspect_media_binding_in_plan(media, &roots)?;
            host.observe_photo_source(media, metadata)?;
        }
        let repair_pause = engine.pause().await;
        for media in &affected {
            engine.invalidate_relinked_media(&repair_pause, &app_paths, &active.namespace(), &previews, &media.media_id)
                .map_err(|failure| failure.message)?;
        }
        drop(repair_pause);
        let mut repair = ImageProcessingBatch::new(affected.len() as u32, |_| {});
        repair.prepare_all_in_plan(&app, affected, roots).await;
        return Err(format!("Não foi possível atualizar a prévia após substituir o original: {error}"));
    }
    let _ = std::fs::remove_file(backup);
    Ok(projection)
    }.await;
    match result {
        Ok(projection) => {
            corrections.finish();
            previews.revoke_viewer_preview(&pending.url);
            let _ = std::fs::remove_file(&pending.path);
            Ok(projection)
        }
        Err(error) => {
            corrections.release_apply();
            let session_active = viewer.current().ok().flatten()
                .is_some_and(|current| current.session_id == session_id);
            if !session_active { let _ = corrections.clear(&previews); }
            Err(error)
        }
    }
}
fn retire(app: &AppHandle, owner: &WebviewWindow, session_id: &str) {
    let store = app.state::<ViewerStore>();
    if store
        .current()
        .ok()
        .flatten()
        .is_some_and(|current| current.session_id != session_id)
    {
        return;
    }
    if let Ok(Some(current)) = store.current()
        && current.session_id == session_id
    {
        let _ = store.clear(session_id);
        let _ = app.state::<CorrectionStore>().clear(&app.state::<CachePreviewRegistry>());
        app.state::<CachePreviewRegistry>().set_viewer_access(false);
        let _ = owner.emit(CLOSED_EVENT, current.session_id);
    }
    native_dialog_window::release_blocked_owner_if_disabled(owner, true);
}

pub(crate) fn retire_for_editor_recovery(app: &AppHandle) -> Result<(), String> {
    let owner = app
        .get_webview_window(PROJECT_WINDOW_LABEL)
        .ok_or("the Project window is unavailable")?;
    if let Some(viewer) = app.get_webview_window(LABEL) {
        native_dialog_window::dismiss_blocked_window(&owner, &viewer, true)
            .map_err(|error| error.to_string())?;
    }
    if let Some(current) = app.state::<ViewerStore>().current()? {
        retire(app, &owner, &current.session_id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applying_correction_keeps_prepared_candidate_until_mutation_finishes() {
        let path = std::env::temp_dir().join(format!("myalbuns-eye-{}.png", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"prepared candidate").unwrap();
        let previews = CachePreviewRegistry::new(LABEL);
        let url = previews.publish_viewer_preview(vec![1, 2, 3]);
        let corrections = CorrectionStore::default();
        assert!(corrections.accept(0, PendingEyes {
            session_id: "session".into(), media_id: "target".into(),
            token: "token".into(), path: path.clone(), url,
            original_path: path.with_extension("jpg"), original_digest: [0; 32],
        }));
        assert!(corrections.claim_for_apply("token", "session").is_some());
        assert!(corrections.clear(&previews).is_none());
        assert!(path.exists(), "cancellation must not unlink a derivative during commit");
        assert!(corrections.is_applying());
        // A failed commit after viewer retirement releases ownership, then discards the orphan.
        corrections.release_apply();
        assert!(corrections.clear(&previews).is_some());
        assert!(!path.exists(), "a failed retired commit must discard its orphan");
    }

    fn presentation(session_id: &str, revision: u64, media_id: &str) -> ViewerPresentation {
        ViewerPresentation {
            session_id: session_id.into(),
            revision,
            media_id: media_id.into(),
            name: media_id.into(),
            url: None,
            state: crate::ipc_contract::ViewerPreviewState::Loading,
            can_previous: false,
            can_next: true,
            correction: None,
        }
    }

    #[test]
    fn viewer_store_rejects_other_sessions_and_stale_updates() {
        let store = ViewerStore::default();
        store.replace(presentation("one", 1, "a")).unwrap();
        store.replace(presentation("one", 3, "c")).unwrap();
        store.replace(presentation("one", 2, "b")).unwrap();
        assert_eq!(store.current().unwrap().unwrap().media_id, "c");
        assert!(store.replace(presentation("two", 1, "d")).is_err());
        assert!(store.clear("one").unwrap());
        store.replace(presentation("two", 1, "d")).unwrap();
    }
}



#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CorrectionAction {
    session_id: String,
    kind: String,
    reference_media_id: Option<String>,
    target_face: Option<Face>,
    reference_face: Option<Face>,
}

#[tauri::command]
pub(crate) fn act_image_viewer_correction(
    app: AppHandle,
    window: WebviewWindow,
    action: CorrectionAction,
    store: State<'_, ViewerStore>,
) -> Result<(), String> {
    if window.label() != LABEL || !matches!(action.kind.as_str(), "start" | "browse" | "select" | "preview" | "apply" | "cancel") {
        return Err("Ação do visualizador inválida.".into());
    }
    if store.current()?.as_ref().is_none_or(|current| current.session_id != action.session_id) {
        return Ok(());
    }
    app.emit_to(PROJECT_WINDOW_LABEL, "myalbuns://image-viewer-correction", action)
        .map_err(|error| error.to_string())
}
