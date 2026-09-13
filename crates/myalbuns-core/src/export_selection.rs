use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{CoreError, ProjectedActiveSides, RectUm, RenderSnapshot};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ExportMode {
    Sheet,
    Page,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
#[ts(tag = "kind")]
pub enum ExportFormat {
    Jpeg { quality: u8 },
    Png,
    Pdf,
}

impl ExportFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Jpeg { .. } => "jpg",
            Self::Png => "png",
            Self::Pdf => "pdf",
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if matches!(self, Self::Jpeg { quality } if !(1..=100).contains(quality)) {
            Err("a qualidade JPEG deve estar entre 1 e 100".into())
        } else {
            Ok(())
        }
    }
}

/// Physical viewport of a selected unit; numbering is from the complete album.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectedExportUnit {
    pub sheet_id: String,
    pub index: usize,
    pub viewport: RectUm,
}

impl RenderSnapshot {
    pub fn export_units(
        &self,
        sheet_ids: &[String],
        mode: ExportMode,
    ) -> Result<Vec<SelectedExportUnit>, CoreError> {
        self.validate()?;
        if sheet_ids.is_empty()
            || sheet_ids
                .iter()
                .enumerate()
                .any(|(index, id)| sheet_ids[..index].contains(id))
        {
            return Err(CoreError::InvalidSnapshot(
                "a seleção de Exportação está vazia ou duplicada".into(),
            ));
        }
        for id in sheet_ids {
            self.output_unit(id)?;
        }
        let positions: Vec<_> = self
            .composition
            .sheets
            .iter()
            .enumerate()
            .filter(|(_, sheet)| sheet_ids.contains(&sheet.sheet_id))
            .map(|(index, _)| index)
            .collect();
        if positions.windows(2).any(|pair| pair[1] != pair[0] + 1) {
            return Err(CoreError::InvalidSnapshot(
                "o Intervalo de Exportação precisa ser contínuo".into(),
            ));
        }
        let mut index = 0;
        let mut selected = Vec::new();
        for sheet in &self.composition.sheets {
            let pages =
                if mode == ExportMode::Page && sheet.active_sides == ProjectedActiveSides::Both {
                    2
                } else {
                    1
                };
            for page in 0..pages {
                index += 1;
                if sheet_ids.contains(&sheet.sheet_id) {
                    let width = sheet.width_um / pages;
                    selected.push(SelectedExportUnit {
                        sheet_id: sheet.sheet_id.clone(),
                        index,
                        viewport: RectUm {
                            x: page * width,
                            y: 0,
                            width,
                            height: sheet.height_um,
                        },
                    });
                }
            }
        }
        Ok(selected)
    }
}
