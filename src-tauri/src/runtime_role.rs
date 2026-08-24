use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

pub(crate) const PROJECT_HOST_ROLE_ARGUMENT: &str = "--myalbuns-project-host";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeRole {
    Global { direct_projects: Vec<PathBuf> },
    ProjectHost,
}

/// Classifies the complete native argument vector, including the executable.
///
/// Association paths remain as `OsString`/`PathBuf` throughout classification;
/// only the ASCII file extension is inspected.
pub(crate) fn parse_runtime_role(arguments: impl IntoIterator<Item = OsString>) -> RuntimeRole {
    let mut direct_projects = Vec::new();

    for argument in arguments.into_iter().skip(1) {
        if argument == OsStr::new(PROJECT_HOST_ROLE_ARGUMENT) {
            return RuntimeRole::ProjectHost;
        }

        if is_project_path(&argument) {
            direct_projects.push(PathBuf::from(argument));
        }
    }

    RuntimeRole::Global { direct_projects }
}

fn is_project_path(argument: &OsStr) -> bool {
    Path::new(argument)
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("myalbuns"))
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    use super::{RuntimeRole, parse_runtime_role};

    fn parse(arguments: impl IntoIterator<Item = OsString>) -> RuntimeRole {
        parse_runtime_role(arguments)
    }

    #[test]
    fn starts_as_global_without_an_associated_project() {
        assert_eq!(
            parse([OsString::from("MyAlbuns.exe")]),
            RuntimeRole::Global {
                direct_projects: Vec::new()
            }
        );
    }

    #[test]
    fn preserves_a_non_ascii_associated_project_path() {
        let project = PathBuf::from(format!(
            "C:\\{}\\{}.myalbuns",
            "\u{00c1}lbuns", "Fam\u{00ed}lia S\u{00e3}o Jo\u{00e3}o"
        ));

        assert_eq!(
            parse([
                OsString::from("MyAlbuns.exe"),
                project.as_os_str().to_owned(),
            ]),
            RuntimeRole::Global {
                direct_projects: vec![project]
            }
        );
    }

    #[test]
    fn recognizes_only_the_fixed_internal_project_host_role() {
        let aliases = ["--host", "--project-host", "--myalbuns-project-host=1"];
        for alias in aliases {
            assert_eq!(
                parse([OsString::from("MyAlbuns.exe"), OsString::from(alias)]),
                RuntimeRole::Global {
                    direct_projects: Vec::new()
                },
                "unexpected internal role alias: {alias}"
            );
        }

        assert_eq!(
            parse([
                OsString::from("MyAlbuns.exe"),
                OsString::from("--myalbuns-project-host"),
            ]),
            RuntimeRole::ProjectHost
        );
    }

    #[test]
    fn ignores_unrelated_flags_and_positionals_but_accepts_the_windows_extension_case() {
        let project = PathBuf::from(r"C:\Projetos\Casamento.MYALBUNS");

        assert_eq!(
            parse([
                OsString::from("MyAlbuns.exe"),
                OsString::from("--verbose"),
                OsString::from("not-a-project.txt"),
                project.as_os_str().to_owned(),
            ]),
            RuntimeRole::Global {
                direct_projects: vec![project]
            }
        );
    }

    #[test]
    fn preserves_every_project_from_one_native_activation() {
        let first = PathBuf::from(r"C:\Projetos\Casamento.myalbuns");
        let second = PathBuf::from(r"\\servidor\Albuns\Formatura.myalbuns");

        let RuntimeRole::Global { direct_projects } = parse([
            OsString::from("MyAlbuns.exe"),
            first.as_os_str().to_owned(),
            OsString::from("--unrelated"),
            second.as_os_str().to_owned(),
        ]) else {
            panic!("a file activation must retain the Global role");
        };

        assert_eq!(direct_projects, vec![first, second]);
    }

    #[test]
    fn the_internal_role_takes_precedence_over_an_association_argument() {
        assert_eq!(
            parse([
                OsString::from("MyAlbuns.exe"),
                OsString::from(r"C:\Projetos\Casamento.myalbuns"),
                OsString::from("--myalbuns-project-host"),
            ]),
            RuntimeRole::ProjectHost
        );
    }

    #[cfg(windows)]
    #[test]
    fn preserves_a_windows_path_that_cannot_be_represented_as_unicode() {
        use std::os::windows::ffi::OsStringExt;

        let mut native = r"C:\Projetos\".encode_utf16().collect::<Vec<_>>();
        native.push(0xd800);
        native.extend(".myalbuns".encode_utf16());
        let project = PathBuf::from(OsString::from_wide(&native));

        assert_eq!(
            parse([
                OsString::from("MyAlbuns.exe"),
                project.as_os_str().to_owned(),
            ]),
            RuntimeRole::Global {
                direct_projects: vec![project]
            }
        );
    }
}
