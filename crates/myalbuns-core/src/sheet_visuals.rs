use crate::{ProjectedBackgroundContent, ProjectedOverlayContent};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DecorativeRole {
    Background,
    Overlay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DecorativeScope {
    BothSides,
    Left,
    Right,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SheetVisualChange {
    BackgroundColor { rgb: String },
    Remove { role: DecorativeRole },
    RestoreAlbum { role: DecorativeRole },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum VisualMapping {
    Side,
    BothSides,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SideVisual<T> {
    #[default]
    Default,
    Custom {
        content: T,
        mapping: VisualMapping,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SheetVisual<T> {
    #[default]
    Default,
    BothSides {
        content: T,
    },
    PerSide {
        left: SideVisual<T>,
        right: SideVisual<T>,
    },
}

impl<T: Clone> SheetVisual<T> {
    pub(crate) fn restore(&mut self, scope: DecorativeScope, active: crate::ActiveSides) {
        if scope == DecorativeScope::BothSides {
            *self = Self::Default;
            return;
        }
        let (mut left, mut right) = match self {
            Self::Default => return,
            Self::BothSides { content } => {
                let side = SideVisual::Custom {
                    content: content.clone(),
                    mapping: VisualMapping::BothSides,
                };
                (side.clone(), side)
            }
            Self::PerSide { left, right } => (left.clone(), right.clone()),
        };
        if scope == DecorativeScope::Left {
            left = SideVisual::Default;
        } else {
            right = SideVisual::Default;
        }
        *self = Self::PerSide { left, right };
        let _ = self.retain_active_sides(active);
    }
    pub(crate) fn retain_active_sides(
        &mut self,
        active: crate::ActiveSides,
    ) -> Option<SideVisual<T>> {
        let mut removed = SideVisual::Default;
        if let Self::PerSide { left, right } = self {
            match active {
                crate::ActiveSides::Both => (),
                crate::ActiveSides::Left => removed = std::mem::take(right),
                crate::ActiveSides::Right => removed = std::mem::take(left),
            }
            if matches!(left, SideVisual::Default) && matches!(right, SideVisual::Default) {
                *self = Self::Default;
            }
        }
        (!matches!(removed, SideVisual::Default)).then_some(removed)
    }

    pub(crate) fn apply(&mut self, scope: DecorativeScope, content: T, active: crate::ActiveSides) {
        if scope == DecorativeScope::BothSides {
            *self = Self::BothSides { content };
            return;
        }
        let (mut left, mut right) = match self {
            Self::Default => (SideVisual::Default, SideVisual::Default),
            Self::BothSides { content } => {
                let side = SideVisual::Custom {
                    content: content.clone(),
                    mapping: VisualMapping::BothSides,
                };
                (side.clone(), side)
            }
            Self::PerSide { left, right } => (left.clone(), right.clone()),
        };
        let target = if scope == DecorativeScope::Left {
            &mut left
        } else {
            &mut right
        };
        *target = SideVisual::Custom {
            content,
            mapping: VisualMapping::Side,
        };
        *self = Self::PerSide { left, right };
        let _ = self.retain_active_sides(active);
    }

    pub(crate) fn contents(&self) -> Vec<&T> {
        match self {
            Self::Default => vec![],
            Self::BothSides { content } => vec![content],
            Self::PerSide { left, right } => [left, right]
                .into_iter()
                .filter_map(|side| match side {
                    SideVisual::Default => None,
                    SideVisual::Custom { content, .. } => Some(content),
                })
                .collect(),
        }
    }

    pub(crate) fn restore_matching(&mut self, matches: impl Fn(&T) -> bool) {
        match self {
            Self::Default => (),
            Self::BothSides { content } => {
                if matches(content) {
                    *self = Self::Default;
                }
            }
            Self::PerSide { left, right } => {
                for side in [&mut *left, &mut *right] {
                    if let SideVisual::Custom { content, .. } = side
                        && matches(content)
                    {
                        *side = SideVisual::Default;
                    }
                }
                if matches!(left, SideVisual::Default) && matches!(right, SideVisual::Default) {
                    *self = Self::Default;
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SheetVisuals {
    pub background: SheetVisual<ProjectedBackgroundContent>,
    pub overlay: SheetVisual<Option<ProjectedOverlayContent>>,
}

impl SheetVisuals {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub(crate) fn retain_active_sides(
        &mut self,
        active: crate::ActiveSides,
    ) -> RemovedSheetVisuals {
        let background = self.background.retain_active_sides(active);
        let overlay = self.overlay.retain_active_sides(active).filter(|side| {
            matches!(
                side,
                SideVisual::Custom {
                    content: Some(_),
                    ..
                }
            )
        });
        RemovedSheetVisuals {
            background,
            overlay,
        }
    }
}

pub(crate) struct RemovedSheetVisuals {
    pub background: Option<SideVisual<ProjectedBackgroundContent>>,
    pub overlay: Option<SideVisual<Option<ProjectedOverlayContent>>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum EdgeConversionSide {
    Left,
    Right,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EdgeConversionLoss {
    pub sheet_id: String,
    pub sheet_number: usize,
    pub side: EdgeConversionSide,
    pub background: Option<SideVisual<ProjectedBackgroundContent>>,
    pub overlay: Option<SideVisual<Option<ProjectedOverlayContent>>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecorativeDropRequest {
    pub sheet_id: String,
    #[ts(type = "string")]
    pub media_id: crate::MediaId,
    pub role: DecorativeRole,
    pub x_um: i64,
    pub y_um: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DecorativeDropPreview {
    pub revision: u64,
    pub role: DecorativeRole,
    pub scope: DecorativeScope,
    pub zone_rect: crate::RectUm,
    pub center_rect: Option<crate::RectUm>,
    pub sheet: crate::ComposedSheet,
}

pub(crate) struct DecorativeDropZone {
    pub scope: DecorativeScope,
    pub rect: crate::RectUm,
    pub center: Option<crate::RectUm>,
}
