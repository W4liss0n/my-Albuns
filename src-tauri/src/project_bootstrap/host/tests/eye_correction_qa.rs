//! Manual QA for eye correction: a saved Project with a corrected
//! derivative reopens and exports through the real Host and Processor.
//! It needs a Project prepared by hand, so no gate runs it.
use super::*;
use crate::{
    export_pipeline::{self, AlbumExportOptions, ExportExecutionControl},
    imaging_processor::InvocationContext,
    imaging_recovery_integration::RealProcessTransport,
    media_runtime::MediaResolver,
    project_host::ProjectHost,
};
use myalbuns_core::{ExportFormat, ExportMode, PhotoPlacementMode, ProjectIntent};
use myalbuns_paths::ExportWriteAuthorization;
use std::{env, fs};

fn environment_path(name: &str) -> PathBuf {
    env::var_os(name).map(PathBuf::from).expect(name)
}

#[test]
#[ignore = "requires MYALBUNS_EYE_QA_PROJECT and a real imaging Processor"]
fn corrected_photo_from_saved_native_project_reopens_and_exports() {
    tauri::async_runtime::block_on(async {
        let saved_project = environment_path("MYALBUNS_EYE_QA_PROJECT");
        let fixture = Fixture::new();
        fs::copy(&saved_project, &fixture.project_path).unwrap();
        let (opened, _) =
            bootstrap_host_project_with_thread(fixture.request(), &fixture.paths).unwrap();
        let HostBootstrap::Ready(opened) = opened else {
            panic!("saved native project opens")
        };
        let host = ProjectHost::new(opened.into_parts().1);
        let corrected = host
            .authorized_media_catalog()
            .unwrap()
            .bindings
            .into_iter()
            .find(|binding| {
                binding
                    .logical_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("nikki-closed-olhos-")
            })
            .expect("the saved native project links its corrected derivative");
        assert!(corrected.logical_path.is_file());
        for binding in host.authorized_media_catalog().unwrap().bindings {
            host.observe_photo_source(
                &binding,
                MediaResolver.inspect_photo_binding(&binding).unwrap(),
            )
            .unwrap();
        }
        let sheet = host.projection().unwrap().state.album.sheets[1].id.clone();
        host.apply_with_outcome(ProjectIntent::AddPhoto {
            sheet_id: sheet.clone(),
            media_id: corrected.media_id.parse().unwrap(),
            mode: PhotoPlacementMode::Normal,
        })
        .unwrap();
        let dpi = host
            .apply_with_outcome(ProjectIntent::SetDpi { dpi: 72 })
            .unwrap()
            .projection;
        host.save(dpi.state.revision).unwrap();
        drop(host);
        let (reopened, _) =
            bootstrap_host_project_with_thread(fixture.request(), &fixture.paths).unwrap();
        let HostBootstrap::Ready(reopened) = reopened else {
            panic!("project with corrected frame reopens")
        };
        let host = ProjectHost::new(reopened.into_parts().1);
        for binding in host.authorized_media_catalog().unwrap().bindings {
            host.observe_photo_source(
                &binding,
                MediaResolver.inspect_photo_binding(&binding).unwrap(),
            )
            .unwrap();
        }
        let (snapshot, sources) = host.freeze_export(std::slice::from_ref(&sheet)).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source_path(), corrected.logical_path.as_path());
        let destination = fixture._root.path().join("export");
        fs::create_dir_all(&destination).unwrap();
        let plan = export_pipeline::plan_album(
            snapshot,
            AlbumExportOptions {
                request_id: "eye-correction-native-qa".into(),
                destination: destination.clone(),
                authorization: ExportWriteAuthorization::CreateOnly,
                sheet_ids: vec![sheet],
                whole_album: false,
                mode: ExportMode::Sheet,
                format: ExportFormat::Jpeg { quality: 90 },
                protected_originals: host
                    .authorized_media_catalog()
                    .unwrap()
                    .bindings
                    .into_iter()
                    .map(|b| b.logical_path)
                    .collect(),
                sources,
            },
        )
        .unwrap();
        let paths = crate::path_io::capture_root_bindings(plan.required_paths())
            .await
            .unwrap();
        let mut transport = RealProcessTransport::stable(
            environment_path("MYALBUNS_REAL_IMAGING_PROCESSOR"),
            fixture._root.path().join("processor-logs"),
        );
        fs::create_dir_all(fixture._root.path().join("processor-logs")).unwrap();
        export_pipeline::execute_album(
            &mut transport,
            plan,
            &paths,
            &ExportExecutionControl::default(),
            &|_| {},
            &InvocationContext::new(
                "eye-correction-native-qa",
                Some(host.projection().unwrap().state.project_id),
            ),
        )
        .await
        .unwrap();
        let exported = fs::read_dir(&destination)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let pixels = image::open(&exported).unwrap().to_rgb8();
        let nonwhite = pixels
            .pixels()
            .filter(|p| p.0.iter().any(|v| *v < 220))
            .count();
        assert!(
            nonwhite > 1000,
            "corrected photo appears in the exported sheet"
        );
        if let Some(evidence) = env::var_os("MYALBUNS_EYE_QA_EXPORT_EVIDENCE") {
            fs::copy(&exported, PathBuf::from(evidence)).unwrap();
        }
        println!(
            "eye correction export: {}x{}, {} nonwhite pixels, source {}",
            pixels.width(),
            pixels.height(),
            nonwhite,
            corrected.logical_path.display()
        );
    });
}
