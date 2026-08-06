use std::path::{Path, PathBuf};

use myalbuns_paths::NativePathDto;
use serde_json::json;

#[test]
fn native_path_dto_round_trips_a_non_ascii_path_through_its_direct_wire_shape() {
    let original = PathBuf::from(r"C:\Álbuns\João\Projeto.myalbuns");
    let dto = NativePathDto::from_path(&original);

    assert_eq!(dto.as_path(), original);

    let wire = serde_json::to_value(&dto).expect("the native pathname is serializable");

    #[cfg(windows)]
    assert_eq!(
        wire,
        json!({
            "encoding": "windowsUtf16",
            "units": [
                67, 58, 92, 193, 108, 98, 117, 110, 115, 92, 74, 111, 227, 111, 92, 80,
                114, 111, 106, 101, 116, 111, 46, 109, 121, 97, 108, 98, 117, 110, 115
            ]
        })
    );

    #[cfg(unix)]
    assert_eq!(
        wire,
        json!({
            "encoding": "unixBytes",
            "bytes": [
                67, 58, 92, 195, 129, 108, 98, 117, 110, 115, 92, 74, 111, 195, 163, 111,
                92, 80, 114, 111, 106, 101, 116, 111, 46, 109, 121, 97, 108, 98, 117, 110, 115
            ]
        })
    );

    let restored: NativePathDto =
        serde_json::from_value(wire).expect("the direct wire shape is reversible");

    assert_eq!(restored.into_path_buf(), original);
}

#[cfg(windows)]
#[test]
fn native_path_dto_preserves_every_windows_utf16_unit_when_taking_pathbuf_ownership() {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};

    let units = vec![67, 58, 92, 111, 112, 97, 113, 117, 101, 45, 0xd800];
    let original = PathBuf::from(OsString::from_wide(&units));
    let dto = NativePathDto::from(original.clone());

    assert_eq!(
        serde_json::to_value(&dto).expect("opaque Windows units are serializable"),
        json!({
            "encoding": "windowsUtf16",
            "units": units
        })
    );

    let restored: NativePathDto = serde_json::from_value(json!({
        "encoding": "windowsUtf16",
        "units": [67, 58, 92, 111, 112, 97, 113, 117, 101, 45, 0xd800]
    }))
    .expect("opaque Windows units are reversible");
    let restored_path: PathBuf = restored.into();

    assert_eq!(restored_path, original);
}

#[test]
fn native_path_dto_accepts_and_exposes_borrowed_paths_without_string_conversion() {
    let original = Path::new(r"C:\Projetos\Álbum.myalbuns");

    let dto = NativePathDto::from(original);
    let exposed: &Path = dto.as_ref();

    assert_eq!(exposed, original);
}
