use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::AppPathsError;

/// The root syntax captured for one external path.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PathRootKind {
    Disk,
    Unc,
    VerbatimDisk,
    VerbatimUnc,
    Posix,
}

/// One immutable logical-to-operational root binding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootBinding {
    kind: PathRootKind,
    #[serde(with = "crate::native_path_serde")]
    logical_root: PathBuf,
    #[serde(with = "crate::native_path_serde")]
    operational_root: PathBuf,
}

impl RootBinding {
    pub fn kind(&self) -> PathRootKind {
        self.kind
    }

    pub fn logical_root(&self) -> &Path {
        &self.logical_root
    }

    pub fn operational_root(&self) -> &Path {
        &self.operational_root
    }
}

/// Frozen bindings shared by every participant in one logical operation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootBindingPlan {
    bindings: Vec<RootBinding>,
}

impl RootBindingPlan {
    pub fn bindings(&self) -> &[RootBinding] {
        &self.bindings
    }

    pub fn validate(&self) -> Result<(), AppPathsError> {
        for (index, binding) in self.bindings.iter().enumerate() {
            let (logical_root, logical_kind) = external_path_root(&binding.logical_root)?;
            validate_operational_base(&binding.operational_root)?;
            if logical_root != binding.logical_root
                || logical_kind != binding.kind
                || self.bindings[..index]
                    .iter()
                    .any(|known| same_root(&known.logical_root, &binding.logical_root))
            {
                return Err(AppPathsError::InvalidOperationPath);
            }
        }
        Ok(())
    }

    /// Resolves a path only through a root captured by the operation owner.
    pub fn resolve(&self, logical_path: &Path) -> Result<PathBuf, AppPathsError> {
        self.validate()?;
        validate_external_path(logical_path)?;
        let (logical_root, _) = external_path_root(logical_path)?;
        let binding = self
            .bindings
            .iter()
            .find(|binding| same_root(&binding.logical_root, &logical_root))
            .ok_or(AppPathsError::PathRootNotBound)?;
        let suffix = strip_root(logical_path, &logical_root)?;
        Ok(binding.operational_root.join(suffix))
    }

    pub fn covers(&self, logical_path: &Path) -> bool {
        self.resolve(logical_path).is_ok()
    }

    pub(crate) fn operational_root_for(&self, logical_path: &Path) -> Result<&Path, AppPathsError> {
        validate_external_path(logical_path)?;
        let (logical_root, _) = external_path_root(logical_path)?;
        self.bindings
            .iter()
            .find(|binding| same_root(&binding.logical_root, &logical_root))
            .map(|binding| binding.operational_root.as_path())
            .ok_or(AppPathsError::PathRootNotBound)
    }
}

/// Mutable collector owned by the command boundary before an operation starts.
#[derive(Debug, Default)]
pub struct OperationPathContext {
    bindings: Vec<RootBinding>,
}

impl OperationPathContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Captures a root once. Its current native spelling becomes the immutable
    /// operational binding consumed by every worker in this attempt.
    pub fn capture(&mut self, path: &Path) -> Result<(), AppPathsError> {
        validate_external_path(path)?;
        let (root, kind) = external_path_root(path)?;
        if self
            .bindings
            .iter()
            .any(|binding| same_root(&binding.logical_root, &root))
        {
            return Ok(());
        }
        #[cfg(windows)]
        let operational_base =
            crate::windows_path::current_operational_base(&root, kind)?.unwrap_or(root);
        #[cfg(not(windows))]
        let operational_base = root;
        self.capture_with_binding(path, &operational_base)
    }

    /// Records a binding resolved by the platform adapter, such as a mapped
    /// drive associated with its current UNC root.
    pub fn capture_with_binding(
        &mut self,
        logical_path: &Path,
        operational_root: &Path,
    ) -> Result<(), AppPathsError> {
        validate_external_path(logical_path)?;
        let (logical_root, kind) = external_path_root(logical_path)?;
        validate_operational_base(operational_root)?;
        if let Some(binding) = self
            .bindings
            .iter()
            .find(|binding| same_root(&binding.logical_root, &logical_root))
        {
            return if same_root(&binding.operational_root, operational_root) {
                Ok(())
            } else {
                Err(AppPathsError::InvalidOperationPath)
            };
        }
        self.bindings.push(RootBinding {
            kind,
            logical_root,
            operational_root: operational_root.to_path_buf(),
        });
        Ok(())
    }

    pub fn freeze(self) -> RootBindingPlan {
        RootBindingPlan {
            bindings: self.bindings,
        }
    }

    pub(crate) fn current_plan(&self) -> RootBindingPlan {
        RootBindingPlan {
            bindings: self.bindings.clone(),
        }
    }
}

pub(crate) fn validate_external_path(path: &Path) -> Result<PathRootKind, AppPathsError> {
    let (_, kind) = external_path_root(path)?;
    let mut normal_component_count = 0;
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {}
            Component::Normal(value) if valid_windows_component(value) => {
                normal_component_count += 1;
            }
            _ => return Err(AppPathsError::InvalidOperationPath),
        }
    }
    if normal_component_count == 0 {
        return Err(AppPathsError::InvalidOperationPath);
    }
    Ok(kind)
}

fn validate_operational_base(path: &Path) -> Result<(), AppPathsError> {
    let (root, _) = external_path_root(path)?;
    if path == root {
        return Ok(());
    }
    validate_external_path(path).map(|_| ())
}

#[cfg(windows)]
fn external_path_root(path: &Path) -> Result<(PathBuf, PathRootKind), AppPathsError> {
    use std::path::Prefix;

    let mut components = path.components();
    let Component::Prefix(prefix_component) = components
        .next()
        .ok_or(AppPathsError::InvalidOperationPath)?
    else {
        return Err(AppPathsError::InvalidOperationPath);
    };
    let kind = match prefix_component.kind() {
        Prefix::Disk(_) => PathRootKind::Disk,
        Prefix::UNC(server, share)
            if valid_windows_component(server) && valid_windows_component(share) =>
        {
            PathRootKind::Unc
        }
        Prefix::VerbatimDisk(_) => PathRootKind::VerbatimDisk,
        Prefix::VerbatimUNC(server, share)
            if valid_windows_component(server) && valid_windows_component(share) =>
        {
            PathRootKind::VerbatimUnc
        }
        Prefix::Verbatim(_) | Prefix::DeviceNS(_) => {
            return Err(AppPathsError::UnsupportedOperationNamespace);
        }
        Prefix::UNC(_, _) | Prefix::VerbatimUNC(_, _) => {
            return Err(AppPathsError::InvalidOperationPath);
        }
    };
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(AppPathsError::InvalidOperationPath);
    }
    let mut root = PathBuf::from(prefix_component.as_os_str());
    root.push(Component::RootDir.as_os_str());
    Ok((root, kind))
}

#[cfg(not(windows))]
fn external_path_root(path: &Path) -> Result<(PathBuf, PathRootKind), AppPathsError> {
    if !path.is_absolute() {
        return Err(AppPathsError::InvalidOperationPath);
    }
    Ok((PathBuf::from("/"), PathRootKind::Posix))
}

fn valid_windows_component(value: &std::ffi::OsStr) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        let units = value.encode_wide().collect::<Vec<_>>();
        if units.is_empty()
            || units == [b'.' as u16]
            || units == [b'.' as u16, b'.' as u16]
            || units
                .last()
                .is_some_and(|unit| *unit == b'.' as u16 || *unit == b' ' as u16)
            || units.iter().any(|unit| {
                *unit <= 0x1f
                    || [
                        b'<' as u16,
                        b'>' as u16,
                        b':' as u16,
                        b'"' as u16,
                        b'/' as u16,
                        b'\\' as u16,
                        b'|' as u16,
                        b'?' as u16,
                        b'*' as u16,
                    ]
                    .contains(unit)
            })
        {
            return false;
        }

        let stem = units
            .split(|unit| *unit == b'.' as u16)
            .next()
            .unwrap_or_default();
        let Some(stem) = stem
            .iter()
            .map(|unit| u8::try_from(*unit).ok())
            .collect::<Option<Vec<_>>>()
            .and_then(|bytes| String::from_utf8(bytes).ok())
        else {
            return true;
        };
        let stem = stem.trim_end_matches(' ').to_ascii_uppercase();
        !is_reserved_windows_stem(&stem)
    }

    #[cfg(not(windows))]
    {
        let Some(value) = value.to_str() else {
            return false;
        };
        if value.is_empty()
            || value == "."
            || value == ".."
            || value.ends_with(['.', ' '])
            || value
                .chars()
                .any(|character| character.is_control() || r#"<>:"/\|?*"#.contains(character))
        {
            return false;
        }

        let stem = value
            .split('.')
            .next()
            .unwrap_or_default()
            .trim_end_matches(' ')
            .to_ascii_uppercase();
        !is_reserved_windows_stem(&stem)
    }
}

fn is_reserved_windows_stem(stem: &str) -> bool {
    matches!(
        stem,
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) || is_numbered_device_alias(stem, "COM")
        || is_numbered_device_alias(stem, "LPT")
}

fn is_numbered_device_alias(value: &str, prefix: &str) -> bool {
    value
        .strip_prefix(prefix)
        .is_some_and(|suffix| matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
}

#[cfg(windows)]
fn strip_root(path: &Path, _root: &Path) -> Result<PathBuf, AppPathsError> {
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Prefix(_)))
        || !matches!(components.next(), Some(Component::RootDir))
    {
        return Err(AppPathsError::PathRootNotBound);
    }
    Ok(components.collect())
}

#[cfg(not(windows))]
fn strip_root(path: &Path, root: &Path) -> Result<PathBuf, AppPathsError> {
    path.strip_prefix(root)
        .map(Path::to_path_buf)
        .map_err(|_| AppPathsError::PathRootNotBound)
}

#[cfg(windows)]
fn same_root(left: &Path, right: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;

    let left = left.as_os_str().encode_wide().collect::<Vec<_>>();
    let right = right.as_os_str().encode_wide().collect::<Vec<_>>();
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| fold_ascii_unit(*left) == fold_ascii_unit(right))
}

#[cfg(not(windows))]
fn same_root(left: &Path, right: &Path) -> bool {
    left == right
}

#[cfg(windows)]
fn fold_ascii_unit(unit: u16) -> u16 {
    if (b'A' as u16..=b'Z' as u16).contains(&unit) {
        unit + (b'a' - b'A') as u16
    } else {
        unit
    }
}
