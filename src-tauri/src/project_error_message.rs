//! Presents typed Core failures at the desktop boundary; diagnostics stay in the log.
use myalbuns_core::CoreError;

pub(crate) fn project_error_message(error: CoreError) -> String {
    tracing::warn!(target: "myalbuns.desktop", error = ?error, event = "project_operation_rejected");
    use CoreError::*;
    match error {
        LayoutLocked => "O layout está travado. Destrave-o no painel de layouts para alterar os quadros.".into(),
        LayoutRequiresLock => "Use o cadeado para aplicar este layout com quadros vazios.".into(),
        LockedLayoutHasNoPlaceholder => "Este layout não tem quadros vazios. Arraste a foto para um quadro preenchido para trocá-la.".into(),
        UnfilledLayoutPositions { .. } => "Adicione fotos aos quadros vazios para exportar a seleção.".into(),
        InvalidLayoutQuery | IncompatibleLayout => "Este layout não pode ser aplicado à lâmina. Escolha outro layout.".into(),
        StaleLayoutPreview => "O layout mudou. Escolha-o novamente antes de aplicar.".into(),
        InvalidFrameBorder => "Confira a cor e a espessura da borda.".into(),
        InvalidFrameStyleSelection | InvalidPhotoEffectSelection | InvalidPhotoZoomSelection
        | InvalidPhotoOrientationSelection | InvalidFrameCopySelection | InvalidFrameDeletionSelection
        | InvalidFrameStackSelection | InvalidFrameGeometrySelection => "Selecione quadros de uma única lâmina para esta ação.".into(),
        InvalidFrameOpacity => "Use uma opacidade entre 0% e 100%.".into(),
        InvalidPhotoAngle => "Use um ângulo entre −45° e 45°, com uma casa decimal.".into(),
        InvalidPhotoZoom => "Use um zoom entre 100% e 500%.".into(),
        InvalidSheetSideSwap => "Só é possível trocar os lados de uma lâmina dupla.".into(),
        FrameClipboardEmpty => "Copie quadros deste projeto antes de colar.".into(),
        InvalidFramePaste => "Os quadros copiados não cabem na área disponível.".into(),
        InvalidFrameContentSwapSelection => "Selecione dois quadros da mesma lâmina, com pelo menos uma foto.".into(),
        InvalidFrameMove => "A foto não foi movida. Solte-a no espaço livre de outra lâmina.".into(),
        FrameGeometryChanged => "O quadro mudou durante o ajuste. Tente novamente.".into(),
        EditableSessionInvalidated => "O projeto não está mais disponível para edição. Reabra-o para continuar.".into(),
        InvalidDpi(dpi) => format!("A resolução de {dpi} DPI não pode ser usada com o tamanho atual do álbum."),
        InvalidAlbumInformation(_) => "Confira os campos das informações do álbum antes de aplicar.".into(),
        AlbumInformationReviewChanged => "A composição mudou. Confira novamente as informações do álbum antes de aplicar.".into(),
        InvalidVisualDefaults => "Confira as opções de aparência do álbum antes de aplicar.".into(),
        FrameNotFound(_) => "O quadro selecionado não está mais disponível. Selecione outro quadro.".into(),
        FrameHasNoPhoto(_) => "Adicione uma foto ao quadro antes de usar esta ação.".into(),
        SheetNotFound(_) => "A lâmina selecionada não está mais disponível. Selecione outra lâmina.".into(),
        InvalidSheetInsertion | InvalidSheetReorder => "Não é possível colocar a lâmina nessa posição.".into(),
        InvalidSheetDuplication => "Só é possível duplicar lâminas duplas.".into(),
        MinimumSheetCount => "O álbum precisa ter pelo menos duas lâminas.".into(),
        InvalidEdgeConversion => "Só é possível alterar o tipo da primeira ou da última lâmina.".into(),
        MediaNotFound(_) => "A imagem selecionada não está mais no projeto. Selecione outra imagem.".into(),
        PlaceholderNotFound(_) => "Esta lâmina não tem quadros vazios para receber a foto.".into(),
        EditableSessionAlreadyOpen { .. } => "Este projeto já está aberto para edição.".into(),
        InvalidPhotoSourceMetadata => "Não foi possível ler as informações da foto.".into(),
        RevisionSpaceExhausted | UnsupportedProjectIntent | InvalidProject(_) | InvalidSnapshot(_)
        | UnsupportedSchema(_) | SavedRevisionMismatch { .. } => "Não foi possível concluir a alteração no projeto.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, EditableProject, ImportPhoto, InitialProject,
        PhotoAngleEdit, PhotoPlacementMode, PhotoSourceMetadata, PhotoZoomEdit, ProjectCore,
        ProjectIntent, ProjectLocation,
    };
    use myalbuns_paths::OperationPathContext;

    #[test]
    fn private_identifiers_and_diagnostics_do_not_become_ui_instructions() {
        let cases = [
            (
                CoreError::FrameNotFound("private-frame-id".into()),
                "private",
            ),
            (
                CoreError::FrameHasNoPhoto("private-frame-id".into()),
                "private",
            ),
            (
                CoreError::SheetNotFound("private-sheet-id".into()),
                "private",
            ),
            (
                CoreError::MediaNotFound("private-media-id".into()),
                "private",
            ),
            (
                CoreError::PlaceholderNotFound("private-sheet-id".into()),
                "private",
            ),
            (
                CoreError::InvalidProject("private document payload".into()),
                "private",
            ),
            (
                CoreError::InvalidSnapshot("private renderer payload".into()),
                "private",
            ),
            (CoreError::UnsupportedSchema(987_654), "987"),
            (
                CoreError::EditableSessionAlreadyOpen {
                    project_id: "private-project-id".into(),
                },
                "private",
            ),
            (
                CoreError::SavedRevisionMismatch {
                    current: 987_654,
                    confirmed: 987_653,
                },
                "987",
            ),
        ];
        for (error, payload) in cases {
            let diagnostic = error.to_string();
            assert!(
                diagnostic.contains(payload),
                "the Core diagnostic carries the payload under test: {diagnostic}"
            );
            let message = project_error_message(error);
            assert!(!message.contains(payload), "{message}");
            assert!(!message.contains("Snapshot"));
        }
    }

    fn project_with_one_photo(root: &std::path::Path) -> (EditableProject, String) {
        let project_path = root.join("Limites.myalbuns");
        let mut context = OperationPathContext::new();
        context.capture(&project_path).unwrap();
        let mut project = ProjectCore::new()
            .with_identity_storage_roots(root.join("leases"), root.join("identities"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .unwrap();
        let original = root.join("Foto.jpg");
        std::fs::write(&original, b"original").unwrap();
        let media_id = project
            .import_photo(ImportPhoto::new(
                original,
                PhotoSourceMetadata::new(
                    600,
                    400,
                    ["#C22C24", "#248044", "#2454C2"].map(String::from),
                )
                .unwrap(),
            ))
            .unwrap()
            .media_id;
        let sheet_id = project.projection().state.album.sheets[0].id.clone();
        project
            .apply(ProjectIntent::AddPhoto {
                sheet_id,
                media_id,
                mode: PhotoPlacementMode::Edit,
            })
            .unwrap();
        let frame_id = project.projection().state.album.sheets[0].frames[0]
            .id
            .clone();
        (project, frame_id)
    }

    #[test]
    fn zoom_guidance_states_the_range_the_core_enforces() {
        let root = tempfile::tempdir().unwrap();
        let (project, frame_id) = project_with_one_photo(root.path());
        let range = project.projection().composition.sheets[0].frames[0]
            .photo
            .as_ref()
            .unwrap()
            .placement
            .zoom_range
            .clone();
        let preview = |user_zoom: f64| {
            project.preview_photo_zoom(&PhotoZoomEdit {
                frame_ids: vec![frame_id.clone()],
                user_zoom: user_zoom as f32,
            })
        };
        assert!(preview(range.minimum).is_ok());
        assert!(preview(range.maximum).is_ok());
        assert_eq!(
            preview(range.minimum - 0.01).unwrap_err(),
            CoreError::InvalidPhotoZoom
        );
        let rejection = preview(range.maximum + 0.01).unwrap_err();
        assert_eq!(rejection, CoreError::InvalidPhotoZoom);

        let percent = |zoom: f64| (zoom * 100.0).round() as u32;
        let message = project_error_message(rejection);
        assert!(
            message.contains(&format!(
                "entre {}% e {}%",
                percent(range.minimum),
                percent(range.maximum)
            )),
            "{message}"
        );
    }

    #[test]
    fn angle_guidance_states_the_range_the_core_enforces() {
        let root = tempfile::tempdir().unwrap();
        let (project, frame_id) = project_with_one_photo(root.path());
        let preview = |angle_tenths: i16| {
            project.preview_photo_angle(&PhotoAngleEdit {
                frame_ids: vec![frame_id.clone()],
                angle_tenths,
            })
        };
        let limit_tenths = (0..=1_800)
            .take_while(|angle_tenths| preview(*angle_tenths).is_ok())
            .last()
            .expect("the neutral angle is accepted");
        assert!(preview(-limit_tenths).is_ok());
        assert_eq!(
            preview(-limit_tenths - 1).unwrap_err(),
            CoreError::InvalidPhotoAngle
        );
        let rejection = preview(limit_tenths + 1).unwrap_err();
        assert_eq!(rejection, CoreError::InvalidPhotoAngle);
        assert_eq!(limit_tenths % 10, 0, "the guidance states whole degrees");

        let degrees = limit_tenths / 10;
        let message = project_error_message(rejection);
        assert!(
            message.contains(&format!("entre \u{2212}{degrees}° e {degrees}°")),
            "{message}"
        );
    }

    #[test]
    fn actionable_domain_rejections_keep_their_distinct_guidance() {
        assert!(project_error_message(CoreError::LayoutLocked).contains("Destrave"));
        assert!(project_error_message(CoreError::LayoutRequiresLock).contains("cadeado"));
        assert!(project_error_message(CoreError::InvalidDpi(1201)).contains("1201 DPI"));
    }
}
