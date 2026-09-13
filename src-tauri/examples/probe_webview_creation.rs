//! Build the production-configured WebView while its disposable HWND cannot
//! take focus, then require a working renderer after the window is restored.

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{path::PathBuf, sync::mpsc, thread, time::Duration};

    use tauri::{LogicalPosition, LogicalSize, WebviewBuilder, WebviewUrl, WindowBuilder};

    if !std::env::args().any(|argument| argument == "--allow-visible-windows") {
        return Err("authorize the desktop and pass --allow-visible-windows".into());
    }
    let directory = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or("an absolute disposable data directory is required")?;
    let _logging = myalbuns_logging::init_local_logging(
        &directory.join("logs"),
        myalbuns_logging::ProcessRole::DesktopHost,
    )?;
    let (sender, receiver) = mpsc::channel();
    myalbuns_desktop_lib::configure_webview_restore_probe(
        tauri::Builder::default(),
        move |result| {
            let _ = sender.send(result);
        },
    )
    .setup(move |app| {
        let mut config = app.config().app.windows.iter()
            .find(|window| window.label == "project")
            .cloned().ok_or("missing production Project configuration")?;
        config.label = "creation-probe".into();
        config.url = WebviewUrl::External("about:blank".parse()?);
        let window = WindowBuilder::new(app, "creation-probe")
            .title("MyAlbuns — teste de criação")
            .inner_size(900.0, 600.0)
            .build()?;
        window.minimize()?;
        if !window.is_minimized()? {
            return Err("the disposable HWND did not minimize".into());
        }
        let webview = window.add_child(
            WebviewBuilder::from_config(&config).data_directory(directory.join("webview")),
            LogicalPosition::new(0.0, 0.0),
            LogicalSize::new(900.0, 600.0),
        )?;
        webview.navigate("about:blank?creation-probe".parse()?)?;
        thread::spawn(move || {
            let probe = || -> Result<(), Box<dyn std::error::Error>> {
                receiver.recv_timeout(Duration::from_secs(10))
                    .map_err(|_| "WebView construction discarded or failed to load the renderer")??;
                window.unminimize()?;
                window.show()?;
                webview.set_focus()?;
                let (sender, receiver) = mpsc::channel();
                webview.eval_with_callback("document.body.textContent = 'ready'; document.body.textContent", move |value| {
                    let _ = sender.send(value);
                })?;
                let value = receiver.recv_timeout(Duration::from_secs(3))?;
                if serde_json::from_str::<String>(&value)? != "ready" {
                    return Err("the restored renderer did not respond".into());
                }
                println!("PASS: production-configured WebView loaded without initial focus and responded after restoration");
                Ok(())
            };
            match probe() {
                Ok(()) => std::process::exit(0),
                Err(error) => {
                    eprintln!("FAIL: {error}");
                    std::process::exit(1);
                }
            }
        });
        Ok(())
    })
    .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This native regression probe requires Windows.");
    std::process::exit(1);
}
