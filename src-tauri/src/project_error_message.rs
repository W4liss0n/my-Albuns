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
        InvalidPhotoZoom => "Use um zoom entre 100% e 400%.".into(),
        InvalidSheetSideSwap => "Só é possível trocar os lados de uma lâmina dupla.".into(),
        FrameClipboardEmpty => "Copie quadros deste projeto antes de colar.".into(),
        InvalidFramePaste => "Os quadros copiados não cabem na área disponível.".into(),
        InvalidFrameContentSwapSelection => "Selecione dois quadros da mesma lâmina, com pelo menos uma foto.".into(),
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

    #[test]
    fn private_identifiers_and_diagnostics_do_not_become_ui_instructions() {
        for error in [
            CoreError::FrameNotFound("private-frame-id".into()),
            CoreError::InvalidSnapshot("private renderer payload".into()),
        ] {
            let message = project_error_message(error);
            assert!(!message.contains("private"));
            assert!(!message.contains("Snapshot"));
        }
    }

    #[test]
    fn actionable_domain_rejections_keep_their_distinct_guidance() {
        assert!(project_error_message(CoreError::LayoutLocked).contains("Destrave"));
        assert!(project_error_message(CoreError::LayoutRequiresLock).contains("cadeado"));
        assert!(project_error_message(CoreError::InvalidDpi(1201)).contains("1201 DPI"));
    }
}
