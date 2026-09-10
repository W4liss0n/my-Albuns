use myalbuns_core::{CustomLayoutId, SaveCustomLayoutResult};
use tauri::{State, WebviewWindow};

use crate::{
    layout_catalog_store::LayoutCatalogStore, product_runtime::PROJECT_WINDOW_LABEL,
    project_host::ProjectHost,
};

#[tauri::command]
pub(crate) async fn refresh_layout_catalog(
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    store: State<'_, LayoutCatalogStore>,
) -> Result<u64, String> {
    authorize(&window)?;
    let snapshot = store
        .load()
        .map_err(|_| "Não foi possível ler os Layouts personalizados.".to_string())?;
    let revision = snapshot.revision;
    state.refresh_layout_catalog(snapshot)?;
    Ok(revision)
}

#[tauri::command]
pub(crate) async fn save_custom_layout(
    sheet_id: String,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    store: State<'_, LayoutCatalogStore>,
) -> Result<SaveCustomLayoutResult, String> {
    authorize(&window)?;
    let definition = state.capture_custom_layout(&sheet_id)?;
    let (result, snapshot) = store
        .create(definition)
        .map_err(|_| "Não foi possível salvar o Layout personalizado.".to_string())?;
    state.refresh_layout_catalog(snapshot)?;
    Ok(result)
}

#[tauri::command]
pub(crate) async fn delete_custom_layout(
    layout_id: CustomLayoutId,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    store: State<'_, LayoutCatalogStore>,
) -> Result<u64, String> {
    authorize(&window)?;
    let snapshot = store
        .delete(layout_id)
        .map_err(|_| "Não foi possível excluir o Layout personalizado.".to_string())?;
    let revision = snapshot.revision;
    state.refresh_layout_catalog(snapshot)?;
    Ok(revision)
}

fn authorize(window: &WebviewWindow) -> Result<(), String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("Os Layouts só podem ser alterados na Janela do Projeto.".into());
    }
    Ok(())
}
