use myalbuns_core::{
    DisplayUnit, EndSheetFormat, InitialBackground as CoreInitialBackground,
    InitialBackgroundContent as CoreInitialBackgroundContent,
    InitialFrameBorder as CoreInitialFrameBorder, InitialOverlay as CoreInitialOverlay,
    InitialOverlayContent as CoreInitialOverlayContent, InitialProject,
    InitialProjectConfiguration as CoreProjectConfiguration, InitialProjectPersonalization,
    ProjectConfigurationValidationError as CoreValidationError, Rgb,
};
use myalbuns_paths::NativePathDto;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum InitialDisplayUnit {
    Mm,
    Cm,
    In,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum InitialSheetFormat {
    Double,
    SinglePage,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InitialDocumentConfiguration {
    pub(crate) display_unit: InitialDisplayUnit,
    pub(crate) sheet_width_um: i64,
    pub(crate) sheet_height_um: i64,
    pub(crate) dpi: i64,
    pub(crate) bleed_um: i64,
    pub(crate) safety_um: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InitialStructureConfiguration {
    pub(crate) sheet_count: i64,
    pub(crate) first_sheet: InitialSheetFormat,
    pub(crate) last_sheet: InitialSheetFormat,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InitialProjectConfiguration {
    pub(crate) document: InitialDocumentConfiguration,
    pub(crate) structure: InitialStructureConfiguration,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum InitialBackgroundContent {
    Color { rgb: String },
    Image { native_path: NativePathDto },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "scope",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum InitialBackground {
    BothSides {
        both: InitialBackgroundContent,
    },
    PerSide {
        left: InitialBackgroundContent,
        right: InitialBackgroundContent,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum InitialOverlayContent {
    Image { native_path: NativePathDto },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "scope",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum InitialOverlay {
    BothSides {
        both: Option<InitialOverlayContent>,
    },
    PerSide {
        left: Option<InitialOverlayContent>,
        right: Option<InitialOverlayContent>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum InitialFrameBorder {
    None,
    Solid { rgb: String, width_um: i64 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InitialVisualDefaults {
    pub(crate) background: InitialBackground,
    pub(crate) overlay: InitialOverlay,
    pub(crate) frame_border: InitialFrameBorder,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InitialProjectCreationConfiguration {
    pub(crate) document: InitialDocumentConfiguration,
    pub(crate) structure: InitialStructureConfiguration,
    pub(crate) visual_defaults: InitialVisualDefaults,
}

impl InitialProjectCreationConfiguration {
    pub(crate) fn dimensions(&self) -> InitialProjectConfiguration {
        InitialProjectConfiguration {
            document: self.document,
            structure: self.structure,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProjectConfigurationValidation {
    errors: Vec<CoreValidationError>,
}

pub(crate) fn validate_configuration(
    configuration: InitialProjectConfiguration,
) -> ProjectConfigurationValidation {
    let errors = to_core_configuration(configuration).validation_errors();
    ProjectConfigurationValidation { errors }
}

pub(crate) fn to_core_configuration(
    configuration: InitialProjectConfiguration,
) -> CoreProjectConfiguration {
    let document = configuration.document;
    let structure = configuration.structure;
    CoreProjectConfiguration::new(
        display_unit(document.display_unit),
        document.sheet_width_um,
        document.sheet_height_um,
        document.dpi,
        document.bleed_um,
        document.safety_um,
        structure.sheet_count,
        end_sheet_format(structure.first_sheet),
        end_sheet_format(structure.last_sheet),
    )
}

pub(crate) fn to_core_initial_project(
    configuration: InitialProjectCreationConfiguration,
) -> Option<InitialProject> {
    let dimensions = configuration.dimensions();
    let personalization = to_core_personalization(configuration.visual_defaults)?;
    Some(
        InitialProject::configured(to_core_configuration(dimensions))
            .with_personalization(personalization),
    )
}

fn to_core_personalization(
    visual_defaults: InitialVisualDefaults,
) -> Option<InitialProjectPersonalization> {
    let background = match visual_defaults.background {
        InitialBackground::BothSides { both } => CoreInitialBackground::BothSides {
            both: to_core_background_content(both)?,
        },
        InitialBackground::PerSide { left, right } => CoreInitialBackground::PerSide {
            left: to_core_background_content(left)?,
            right: to_core_background_content(right)?,
        },
    };
    let overlay = match visual_defaults.overlay {
        InitialOverlay::BothSides { both } => CoreInitialOverlay::BothSides {
            both: both.map(to_core_overlay_content),
        },
        InitialOverlay::PerSide { left, right } => CoreInitialOverlay::PerSide {
            left: left.map(to_core_overlay_content),
            right: right.map(to_core_overlay_content),
        },
    };
    let frame_border = match visual_defaults.frame_border {
        InitialFrameBorder::None => CoreInitialFrameBorder::None,
        InitialFrameBorder::Solid { rgb, width_um } => CoreInitialFrameBorder::Solid {
            rgb: Rgb::parse_canonical(&rgb)?,
            width_um,
        },
    };
    Some(InitialProjectPersonalization::new(
        background,
        overlay,
        frame_border,
    ))
}

fn to_core_background_content(
    content: InitialBackgroundContent,
) -> Option<CoreInitialBackgroundContent> {
    Some(match content {
        InitialBackgroundContent::Color { rgb } => CoreInitialBackgroundContent::Color {
            rgb: Rgb::parse_canonical(&rgb)?,
        },
        InitialBackgroundContent::Image { native_path } => CoreInitialBackgroundContent::Media {
            path: native_path.into_path_buf(),
        },
    })
}

fn to_core_overlay_content(content: InitialOverlayContent) -> CoreInitialOverlayContent {
    match content {
        InitialOverlayContent::Image { native_path } => CoreInitialOverlayContent::Media {
            path: native_path.into_path_buf(),
        },
    }
}

fn display_unit(unit: InitialDisplayUnit) -> DisplayUnit {
    match unit {
        InitialDisplayUnit::Mm => DisplayUnit::Mm,
        InitialDisplayUnit::Cm => DisplayUnit::Cm,
        InitialDisplayUnit::In => DisplayUnit::In,
    }
}

fn end_sheet_format(format: InitialSheetFormat) -> EndSheetFormat {
    match format {
        InitialSheetFormat::Double => EndSheetFormat::Double,
        InitialSheetFormat::SinglePage => EndSheetFormat::SinglePage,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    fn valid_configuration() -> InitialProjectConfiguration {
        InitialProjectConfiguration {
            document: InitialDocumentConfiguration {
                display_unit: InitialDisplayUnit::Cm,
                sheet_width_um: 508_000,
                sheet_height_um: 254_000,
                dpi: 240,
                bleed_um: 4_000,
                safety_um: 7_500,
            },
            structure: InitialStructureConfiguration {
                sheet_count: 3,
                first_sheet: InitialSheetFormat::SinglePage,
                last_sheet: InitialSheetFormat::Double,
            },
        }
    }

    fn personalized_creation() -> InitialProjectCreationConfiguration {
        let dimensions = valid_configuration();
        InitialProjectCreationConfiguration {
            document: dimensions.document,
            structure: dimensions.structure,
            visual_defaults: InitialVisualDefaults {
                background: InitialBackground::PerSide {
                    left: InitialBackgroundContent::Color {
                        rgb: "#102030".into(),
                    },
                    right: InitialBackgroundContent::Image {
                        native_path: NativePathDto::from(PathBuf::from(r"C:\Imagens\Fundo 🌳.png")),
                    },
                },
                overlay: InitialOverlay::BothSides {
                    both: Some(InitialOverlayContent::Image {
                        native_path: NativePathDto::from(PathBuf::from(
                            "C:\\Imagens\\Sobreposi\u{e7}\u{e3}o.png",
                        )),
                    }),
                },
                frame_border: InitialFrameBorder::Solid {
                    rgb: "#A0B0C0".into(),
                    width_um: 1_250,
                },
            },
        }
    }

    #[test]
    fn valid_configuration_has_no_structural_errors() {
        assert_eq!(
            validate_configuration(valid_configuration()),
            ProjectConfigurationValidation { errors: vec![] }
        );
    }

    #[test]
    fn creation_configuration_keeps_reversible_native_paths_and_maps_to_core() {
        let configuration = personalized_creation();
        let encoded = serde_json::to_value(&configuration)
            .expect("the personalized configuration serializes");

        assert!(
            encoded["visualDefaults"]["background"]["right"]["nativePath"].is_object(),
            "native pathnames must never be flattened into lossy strings"
        );
        let decoded: InitialProjectCreationConfiguration =
            serde_json::from_value(encoded).expect("the personalized configuration round-trips");
        assert_eq!(decoded, configuration);
        assert!(to_core_initial_project(decoded).is_some());
    }

    #[test]
    fn creation_configuration_rejects_non_canonical_colors_before_core_creation() {
        let mut configuration = personalized_creation();
        configuration.visual_defaults.frame_border = InitialFrameBorder::Solid {
            rgb: "#a0b0c0".into(),
            width_um: 1_250,
        };

        assert!(to_core_initial_project(configuration).is_none());
    }

    #[test]
    fn negative_and_invalid_fields_return_all_independent_errors_in_wire_order() {
        let invalid = InitialProjectConfiguration {
            document: InitialDocumentConfiguration {
                display_unit: InitialDisplayUnit::In,
                sheet_width_um: -2,
                sheet_height_um: -1,
                dpi: 0,
                bleed_um: -3,
                safety_um: -4,
            },
            structure: InitialStructureConfiguration {
                sheet_count: 1,
                first_sheet: InitialSheetFormat::SinglePage,
                last_sheet: InitialSheetFormat::Double,
            },
        };

        assert_eq!(
            serde_json::to_value(validate_configuration(invalid))
                .expect("the validation response serializes"),
            json!({
                "errors": [
                    "sheetWidthNotPositive",
                    "sheetHeightNotPositive",
                    "dpiOutOfRange",
                    "sheetCountTooSmall",
                    "bleedNegative",
                    "safetyNegative"
                ]
            })
        );
    }

    #[test]
    fn validation_response_and_error_enum_are_closed() {
        assert!(
            serde_json::from_value::<ProjectConfigurationValidation>(json!({
                "errors": [],
                "futureOption": true
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<ProjectConfigurationValidation>(json!({
                "errors": ["futureError"]
            }))
            .is_err()
        );
    }

    #[test]
    fn every_core_validation_error_has_the_expected_camel_case_wire_code() {
        let cases = [
            (
                CoreValidationError::SheetWidthNotPositive,
                "sheetWidthNotPositive",
            ),
            (
                CoreValidationError::SheetWidthAboveSafeInteger,
                "sheetWidthAboveSafeInteger",
            ),
            (CoreValidationError::SheetWidthNotEven, "sheetWidthNotEven"),
            (
                CoreValidationError::SheetWidthRasterOutOfRange,
                "sheetWidthRasterOutOfRange",
            ),
            (
                CoreValidationError::SheetHeightNotPositive,
                "sheetHeightNotPositive",
            ),
            (
                CoreValidationError::SheetHeightAboveSafeInteger,
                "sheetHeightAboveSafeInteger",
            ),
            (
                CoreValidationError::SheetHeightRasterOutOfRange,
                "sheetHeightRasterOutOfRange",
            ),
            (CoreValidationError::DpiOutOfRange, "dpiOutOfRange"),
            (
                CoreValidationError::SheetCountTooSmall,
                "sheetCountTooSmall",
            ),
            (CoreValidationError::BleedNegative, "bleedNegative"),
            (
                CoreValidationError::BleedAboveSafeInteger,
                "bleedAboveSafeInteger",
            ),
            (
                CoreValidationError::BleedEliminatesCutArea,
                "bleedEliminatesCutArea",
            ),
            (CoreValidationError::SafetyNegative, "safetyNegative"),
            (
                CoreValidationError::SafetyAboveSafeInteger,
                "safetyAboveSafeInteger",
            ),
            (
                CoreValidationError::SafetyEliminatesSafeArea,
                "safetyEliminatesSafeArea",
            ),
        ];

        for (error, expected_code) in cases {
            let encoded =
                serde_json::to_value(error).expect("the canonical validation error serializes");
            assert_eq!(encoded, expected_code);
        }
    }
}
