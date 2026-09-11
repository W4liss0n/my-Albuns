//! Native regression probe. Only controls its own disposable WebView window.
//! Run with an absolute, disposable WebView data directory as the first argument.

#[cfg(windows)]
#[allow(dead_code)]
#[path = "../src/desktop_webview_policy.rs"]
mod desktop_webview_policy;

#[cfg(windows)]
fn evaluate(
    window: &tauri::WebviewWindow,
    script: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    use std::{sync::mpsc, time::Duration};

    let (sender, receiver) = mpsc::channel();
    window.eval_with_callback(script, move |result| {
        let _ = sender.send(result);
    })?;
    let result = receiver
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| "the WebView renderer stopped responding")?;
    Ok(serde_json::from_str(&result)?)
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{path::PathBuf, sync::mpsc, thread, time::Duration};

    use tauri::{WebviewUrl, WebviewWindowBuilder};

    if !std::env::args().any(|argument| argument == "--allow-visible-windows") {
        return Err("authorize the desktop and pass --allow-visible-windows".into());
    }
    let data_directory = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or("an absolute disposable WebView data directory is required")?;
    tauri::Builder::default()
        .manage(desktop_webview_policy::WindowWebviewVisibility::default())
        .on_window_event(desktop_webview_policy::on_window_event)
        .setup(move |app| {
            let window = WebviewWindowBuilder::new(
                app,
                "restore-probe",
                WebviewUrl::External("about:blank".parse()?),
            )
            .title("MyAlbuns — teste de restauração")
            .inner_size(900.0, 600.0)
            .decorations(false)
            .data_directory(data_directory)
            .build()?;
            desktop_webview_policy::enforce_webview(window.as_ref())?;

            thread::spawn(move || {
                let probe = || -> Result<(), Box<dyn std::error::Error>> {
                    thread::sleep(Duration::from_millis(500));
                    evaluate(
                        &window,
                        "globalThis.restoreProbe = { size: [innerWidth, innerHeight], events: [] };\
                         document.addEventListener('visibilitychange', () => {\
                           restoreProbe.events.push({ visible: !document.hidden, size: [innerWidth, innerHeight] });\
                         }); true;",
                    )?;
                    for cycle in 1..=3 {
                        window.minimize()?;
                        thread::sleep(Duration::from_millis(400));
                        let (sender, receiver) = mpsc::channel();
                        let state_window = window.clone();
                        window.run_on_main_thread(move || {
                            let _ = sender.send(state_window.is_minimized());
                        })?;
                        let minimized = receiver.recv_timeout(Duration::from_secs(3)).map_err(|_| {
                            format!("cycle {cycle}: native event loop stopped after minimize")
                        })??;
                        if !minimized {
                            return Err(format!("cycle {cycle}: the probe did not minimize").into());
                        }

                        window.unminimize()?;
                        window.set_focus()?;
                        thread::sleep(Duration::from_millis(400));
                        let (sender, receiver) = mpsc::channel();
                        let state_window = window.clone();
                        window.run_on_main_thread(move || {
                            let _ = sender.send(state_window.is_minimized());
                        })?;
                        let minimized = receiver.recv_timeout(Duration::from_secs(3)).map_err(|_| {
                            format!("cycle {cycle}: native event loop stopped after restore")
                        })??;
                        if minimized {
                            return Err(format!("cycle {cycle}: the probe did not restore").into());
                        }
                        println!("PASS: minimize/restore cycle {cycle} kept the event loop responsive");
                        let (sender, receiver) = mpsc::channel();
                        window.with_webview(move |native| {
                            let state = || -> windows::core::Result<_> {
                                let controller = native.controller();
                                let mut visible = windows::core::BOOL::default();
                                let mut bounds = windows::Win32::Foundation::RECT::default();
                                unsafe {
                                    controller.IsVisible(&mut visible)?;
                                    controller.Bounds(&mut bounds)?;
                                }
                                Ok((visible.as_bool(), bounds.right, bounds.bottom))
                            };
                            let _ = sender.send(state().map_err(|error| error.to_string()));
                        })?;
                        let state = receiver.recv_timeout(Duration::from_secs(3))??;
                        if !state.0 {
                            return Err(format!("cycle {cycle}: restored controller remained hidden").into());
                        }
                        println!("Controller after restore {cycle}: {state:?}");
                        let rendered = evaluate(&window, "restoreProbe")?;
                        println!("Renderer after restore {cycle}: {rendered}");
                    }
                    let rendered = evaluate(&window, "restoreProbe")?;
                    let expected = &rendered["size"];
                    let events = rendered["events"].as_array().ok_or("missing visibility events")?;
                    let restored = events.iter().filter(|event| event["visible"] == true).collect::<Vec<_>>();
                    if restored.len() < 3 {
                        return Err(format!("expected three renderer restores: {rendered}").into());
                    }
                    for event in restored {
                        if &event["size"] != expected {
                            return Err(format!("restored viewport collapsed: {rendered}").into());
                        }
                    }
                    println!("PASS: every restored renderer exposed the complete viewport: {rendered}");
                    window.set_size(tauri::LogicalSize::new(1000.0, 700.0))?;
                    thread::sleep(Duration::from_millis(400));
                    let resized = evaluate(&window, "[innerWidth, innerHeight]")?;
                    if resized != serde_json::json!([1000, 700]) {
                        return Err(format!("automatic resize did not resume: {resized}").into());
                    }
                    println!("PASS: ordinary window resize resumed after restoration");
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
