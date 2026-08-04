use myalbuns_core::{
    DisplayUnit, EndSheetFormat, InitialProjectConfiguration as CoreProjectConfiguration,
    InitialProjectValidationError as CoreValidationError,
};
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProjectConfigurationValidation {
    errors: Vec<ProjectConfigurationValidationError>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum ProjectConfigurationValidationError {
    SheetWidthNotPositive,
    SheetWidthAboveSafeInteger,
    SheetWidthNotEven,
    SheetWidthRasterOutOfRange,
    SheetHeightNotPositive,
    SheetHeightAboveSafeInteger,
    SheetHeightRasterOutOfRange,
    DpiOutOfRange,
    SheetCountTooSmall,
    BleedNegative,
    BleedAboveSafeInteger,
    BleedEliminatesCutArea,
    SafetyNegative,
    SafetyAboveSafeInteger,
    SafetyEliminatesSafeArea,
}

pub(crate) fn validate_configuration(
    configuration: InitialProjectConfiguration,
) -> ProjectConfigurationValidation {
    let errors = to_core_configuration(configuration)
        .validation_errors()
        .into_iter()
        .map(validation_error)
        .collect();
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

fn validation_error(error: CoreValidationError) -> ProjectConfigurationValidationError {
    match error {
        CoreValidationError::SheetWidthNotPositive => {
            ProjectConfigurationValidationError::SheetWidthNotPositive
        }
        CoreValidationError::SheetWidthAboveSafeInteger => {
            ProjectConfigurationValidationError::SheetWidthAboveSafeInteger
        }
        CoreValidationError::SheetWidthNotEven => {
            ProjectConfigurationValidationError::SheetWidthNotEven
        }
        CoreValidationError::SheetWidthRasterOutOfRange => {
            ProjectConfigurationValidationError::SheetWidthRasterOutOfRange
        }
        CoreValidationError::SheetHeightNotPositive => {
            ProjectConfigurationValidationError::SheetHeightNotPositive
        }
        CoreValidationError::SheetHeightAboveSafeInteger => {
            ProjectConfigurationValidationError::SheetHeightAboveSafeInteger
        }
        CoreValidationError::SheetHeightRasterOutOfRange => {
            ProjectConfigurationValidationError::SheetHeightRasterOutOfRange
        }
        CoreValidationError::DpiOutOfRange => ProjectConfigurationValidationError::DpiOutOfRange,
        CoreValidationError::SheetCountTooSmall => {
            ProjectConfigurationValidationError::SheetCountTooSmall
        }
        CoreValidationError::BleedNegative => ProjectConfigurationValidationError::BleedNegative,
        CoreValidationError::BleedAboveSafeInteger => {
            ProjectConfigurationValidationError::BleedAboveSafeInteger
        }
        CoreValidationError::BleedEliminatesCutArea => {
            ProjectConfigurationValidationError::BleedEliminatesCutArea
        }
        CoreValidationError::SafetyNegative => ProjectConfigurationValidationError::SafetyNegative,
        CoreValidationError::SafetyAboveSafeInteger => {
            ProjectConfigurationValidationError::SafetyAboveSafeInteger
        }
        CoreValidationError::SafetyEliminatesSafeArea => {
            ProjectConfigurationValidationError::SafetyEliminatesSafeArea
        }
    }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn valid_configuration_has_no_structural_errors() {
        assert_eq!(
            validate_configuration(valid_configuration()),
            ProjectConfigurationValidation { errors: vec![] }
        );
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
            let encoded = serde_json::to_value(validation_error(error))
                .expect("the mapped validation error serializes");
            assert_eq!(encoded, expected_code);
        }
    }
}
