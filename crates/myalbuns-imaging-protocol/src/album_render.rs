use crate::{IMAGING_PROTOCOL_VERSION, RenderCompletion, RenderSource, is_safe_identifier};
use myalbuns_core::{RenderSnapshot, SelectedExportUnit};
use myalbuns_paths::{NativePathDto, RootBindingPlan};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub use myalbuns_core::ExportFormat as RenderFormat;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlbumRenderOutput {
    pub prepared_path: NativePathDto,
    pub units: Vec<SelectedExportUnit>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlbumRenderRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub snapshot: RenderSnapshot,
    pub format: RenderFormat,
    pub outputs: Vec<AlbumRenderOutput>,
    pub sources: Vec<RenderSource>,
    pub root_bindings: RootBindingPlan,
}

impl AlbumRenderRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.protocol_version != IMAGING_PROTOCOL_VERSION
            || !is_safe_identifier(&self.request_id)
        {
            return Err("a versão ou correlação da Exportação é inválida".into());
        }
        self.snapshot
            .validate()
            .map_err(|error| error.to_string())?;
        self.format.validate()?;
        self.root_bindings
            .validate()
            .map_err(|error| error.to_string())?;
        if self.outputs.is_empty() || (self.format == RenderFormat::Pdf && self.outputs.len() != 1)
        {
            return Err("a Exportação não tem um conjunto válido de saídas".into());
        }
        let mut required = HashSet::new();
        let mut paths = HashSet::new();
        let mut indexes = HashSet::new();
        for output in &self.outputs {
            let path = output.prepared_path.as_path();
            if !path.is_absolute()
                || !paths.insert(path)
                || !self.root_bindings.covers(path)
                || path.extension().and_then(|value| value.to_str())
                    != Some(self.format.extension())
                || output.units.is_empty()
                || (self.format != RenderFormat::Pdf && output.units.len() != 1)
            {
                return Err("a preparação da Exportação é inválida".into());
            }
            for unit in &output.units {
                let sheet = self
                    .snapshot
                    .output_unit(&unit.sheet_id)
                    .map_err(|error| error.to_string())?
                    .sheet;
                let view = &unit.viewport;
                let full = view.x == 0
                    && view.y == 0
                    && view.width == sheet.width_um
                    && view.height == sheet.height_um;
                let page = sheet.active_sides == myalbuns_core::ProjectedActiveSides::Both
                    && view.y == 0
                    && view.height == sheet.height_um
                    && view.width == sheet.width_um / 2
                    && (view.x == 0 || view.x == view.width);
                if (!full && !page) || unit.index == 0 || !indexes.insert(unit.index) {
                    return Err("a unidade de Exportação não pertence à composição".into());
                }
                required.extend(sheet.referenced_media_ids());
            }
        }
        let supplied: HashSet<_> = self.sources.iter().map(RenderSource::media_id).collect();
        let units: Vec<_> = self
            .outputs
            .iter()
            .flat_map(|output| output.units.iter())
            .collect();
        let mut sheets = Vec::new();
        for unit in &units {
            if !sheets.contains(&unit.sheet_id) {
                sheets.push(unit.sheet_id.clone());
            }
        }
        let canonical = [
            myalbuns_core::ExportMode::Sheet,
            myalbuns_core::ExportMode::Page,
        ]
        .into_iter()
        .any(|mode| {
            self.snapshot
                .export_units(&sheets, mode)
                .is_ok_and(|expected| expected.iter().eq(units.iter().copied()))
        });
        if !canonical {
            return Err(
                "a ordem, a numeração ou o intervalo das unidades de Exportação é inválido".into(),
            );
        }
        if supplied.len() != self.sources.len()
            || supplied != required
            || self
                .sources
                .iter()
                .any(|source| !self.root_bindings.covers(source.source_path()))
        {
            return Err("as fontes da Exportação não correspondem à seleção".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlbumRenderCompletion {
    pub outputs: Vec<RenderCompletion>,
}
