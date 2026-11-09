//! The active application in a Wayland session, asked to the compositor.
//!
//! Wayland has no standard way to know the focused window, so each compositor is handled on its own:
//! * KWin (KDE Plasma): a small KWin script reports every window activation to ExpanderCraft over D-Bus.
//! * Sway: the i3-compatible IPC socket (`GET_TREE`).
//! * Hyprland: the IPC socket (`activewindow`).
//!
//! With other compositors (GNOME...) only X11 (XWayland) windows are recognized.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::Value;

/// The focused window, as reported by the compositor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveApp {
    /// The process name (`/proc/<pid>/comm`), or the application id when the process is unknown.
    pub name: String,
    pub pid: Option<u32>,
}

impl ActiveApp {
    fn new(class: &str, pid: Option<u32>) -> Option<ActiveApp> {
        let comm = pid.filter(|p| *p > 0).and_then(|p| std::fs::read_to_string(format!("/proc/{p}/comm")).ok());
        let name = comm.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| class.to_string());
        (!name.is_empty()).then_some(ActiveApp { name, pid })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    KWin,
    Sway,
    Hyprland,
}

static SOURCE: OnceLock<Option<Source>> = OnceLock::new();
static KWIN_ACTIVE: Mutex<Option<ActiveApp>> = Mutex::new(None);
static KWIN_CONNECTION: OnceLock<zbus::blocking::Connection> = OnceLock::new();

const BUS_NAME: &str = "io.github.expandercraft";
const OBJECT_PATH: &str = "/ActiveWindow";
const INTERFACE: &str = "io.github.expandercraft.ActiveWindow";
const KWIN_PLUGIN: &str = "expandercraft";

/// Detect the compositor and start following the active window. Called once, at startup.
pub fn start() {
    let source = if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        Some(Source::Hyprland)
    } else if std::env::var_os("SWAYSOCK").is_some() {
        Some(Source::Sway)
    } else if std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_ascii_uppercase().contains("KDE") {
        match start_kwin() {
            Ok(()) => Some(Source::KWin),
            Err(e) => {
                log::warn!("The KWin script for the active window could not be loaded: {e}");
                None
            }
        }
    } else {
        None
    };
    log::info!("Active window: {}", source.map(|s| format!("{s:?}")).unwrap_or_else(|| "X11 windows only".into()));
    let _ = SOURCE.set(source);
}

/// `None` if the compositor is not supported; `Some(None)` if no window is focused.
pub fn active_app() -> Option<Option<ActiveApp>> {
    match (*SOURCE.get()?)? {
        Source::KWin => Some(KWIN_ACTIVE.lock().clone()),
        Source::Sway => Some(sway_active()),
        Source::Hyprland => Some(hyprland_active()),
    }
}

/// Unload the KWin script (when quitting).
pub fn stop() {
    if let Some(conn) = KWIN_CONNECTION.get()
        && let Ok(kwin) = zbus::blocking::Proxy::new(conn, "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting")
    {
        let _: zbus::Result<bool> = kwin.call("unloadScript", &(KWIN_PLUGIN,));
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// KWin
// ---------------------------------------------------------------------------------------------------------------------

/// Receives the activations reported by the KWin script.
struct ActiveWindowListener;

#[zbus::interface(name = "io.github.expandercraft.ActiveWindow")]
impl ActiveWindowListener {
    fn update(&self, class: String, pid: String) {
        *KWIN_ACTIVE.lock() = ActiveApp::new(&class, pid.parse().ok());
    }
}

/// Works with KWin 5 (`clientActivated`) and 6 (`windowActivated`). The pid is sent as a string so that
/// the D-Bus signature does not depend on how KWin converts JavaScript numbers.
fn kwin_script() -> String {
    format!(
        r#"(function () {{
    function report(w) {{
        callDBus("{BUS_NAME}", "{OBJECT_PATH}", "{INTERFACE}", "Update",
                 w ? String(w.resourceClass || "") : "", w ? String(w.pid || 0) : "0");
    }}
    var activated = workspace.windowActivated || workspace.clientActivated;
    activated.connect(report);
    report(workspace.activeWindow || workspace.activeClient);
}})();
"#
    )
}

fn start_kwin() -> zbus::Result<()> {
    let conn = zbus::blocking::connection::Builder::session()?.name(BUS_NAME)?.serve_at(OBJECT_PATH, ActiveWindowListener)?.build()?;
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let path = dir.join("expandercraft-kwin.js");
    std::fs::write(&path, kwin_script()).map_err(|e| zbus::Error::Failure(e.to_string()))?;

    let kwin = zbus::blocking::Proxy::new(&conn, "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting")?;
    // a script left by a run that did not quit properly still reports to the same D-Bus name: keep it
    let loaded: zbus::Result<bool> = kwin.call("isScriptLoaded", &(KWIN_PLUGIN,));
    if loaded.unwrap_or(false) {
        log::info!("The KWin script is already loaded.");
        let _ = KWIN_CONNECTION.set(conn);
        return Ok(());
    }
    let id: i32 = kwin.call("loadScript", &(path.display().to_string(), KWIN_PLUGIN))?;
    if id < 0 {
        return Err(zbus::Error::Failure("loadScript failed".into()));
    }
    // Plasma 6 path, then Plasma 5
    let run = |object: String| -> zbus::Result<()> {
        let script = zbus::blocking::Proxy::new(&conn, "org.kde.KWin", object, "org.kde.kwin.Script")?;
        script.call_method("run", &())?;
        Ok(())
    };
    run(format!("/Scripting/Script{id}")).or_else(|_| run(format!("/{id}")))?;
    let _ = KWIN_CONNECTION.set(conn);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// Sway
// ---------------------------------------------------------------------------------------------------------------------

fn sway_active() -> Option<ActiveApp> {
    let socket = std::env::var_os("SWAYSOCK")?;
    let mut s = UnixStream::connect(socket).ok()?;
    s.set_read_timeout(Some(Duration::from_millis(300))).ok()?;
    const GET_TREE: u32 = 4;
    let mut msg = b"i3-ipc".to_vec();
    msg.extend_from_slice(&0u32.to_ne_bytes());
    msg.extend_from_slice(&GET_TREE.to_ne_bytes());
    s.write_all(&msg).ok()?;
    let mut header = [0u8; 14];
    s.read_exact(&mut header).ok()?;
    let len = u32::from_ne_bytes(header[6..10].try_into().ok()?) as usize;
    let mut body = vec![0u8; len];
    s.read_exact(&mut body).ok()?;
    let tree: Value = serde_json::from_slice(&body).ok()?;
    sway_focused(&tree)
}

/// The focused window of a sway tree.
pub fn sway_focused(node: &Value) -> Option<ActiveApp> {
    if node.get("focused").and_then(Value::as_bool) == Some(true) && node.get("pid").is_some() {
        let class = node
            .get("app_id")
            .and_then(Value::as_str)
            .or_else(|| node.pointer("/window_properties/class").and_then(Value::as_str))
            .unwrap_or("");
        let pid = node.get("pid").and_then(Value::as_u64).map(|p| p as u32);
        return ActiveApp::new(class, pid);
    }
    ["nodes", "floating_nodes"].iter().filter_map(|k| node.get(*k).and_then(Value::as_array)).flatten().find_map(sway_focused)
}

// ---------------------------------------------------------------------------------------------------------------------
// Hyprland
// ---------------------------------------------------------------------------------------------------------------------

fn hyprland_request(command: &str) -> Option<String> {
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        candidates.push(PathBuf::from(dir).join("hypr").join(&sig).join(".socket.sock"));
    }
    candidates.push(PathBuf::from("/tmp/hypr").join(&sig).join(".socket.sock"));
    let mut s = candidates.iter().find_map(|p| UnixStream::connect(p).ok())?;
    s.set_read_timeout(Some(Duration::from_millis(300))).ok()?;
    s.write_all(command.as_bytes()).ok()?;
    let mut out = String::new();
    s.read_to_string(&mut out).ok()?;
    Some(out)
}

fn hyprland_active() -> Option<ActiveApp> {
    let v: Value = serde_json::from_str(&hyprland_request("j/activewindow")?).ok()?;
    let class = v.get("class").and_then(Value::as_str).unwrap_or("");
    let pid = v.get("pid").and_then(Value::as_i64).filter(|p| *p > 0).map(|p| p as u32);
    ActiveApp::new(class, pid)
}

/// The `input:kb_layout` option of Hyprland.
pub fn hyprland_layout() -> Option<String> {
    let v: Value = serde_json::from_str(&hyprland_request("j/getoption input:kb_layout")?).ok()?;
    v.get("str").and_then(Value::as_str).map(str::to_string).filter(|s| !s.is_empty() && s != "[[EMPTY]]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sway_tree() {
        let tree = serde_json::json!({
            "focused": false, "nodes": [
                {"focused": false, "nodes": [
                    {"focused": false, "pid": 10, "app_id": "foot", "nodes": []},
                    {"focused": true, "pid": 0, "app_id": null, "window_properties": {"class": "Gimp"}, "nodes": []}
                ], "floating_nodes": []}
            ]
        });
        assert_eq!(sway_focused(&tree), Some(ActiveApp { name: "Gimp".into(), pid: Some(0) }));
    }

    /// A stand-in for KWin's scripting D-Bus API: loading and running the script reports a window.
    struct FakeScripting;

    #[zbus::interface(name = "org.kde.kwin.Scripting")]
    impl FakeScripting {
        #[zbus(name = "loadScript")]
        fn load_script(&self, path: String, plugin: String) -> i32 {
            assert_eq!(plugin, KWIN_PLUGIN);
            assert!(std::fs::read_to_string(path).unwrap().contains("callDBus"));
            7
        }
        #[zbus(name = "unloadScript")]
        fn unload_script(&self, _plugin: String) -> bool {
            true
        }
        #[zbus(name = "isScriptLoaded")]
        fn is_script_loaded(&self, _plugin: String) -> bool {
            false
        }
    }

    struct FakeScript;

    #[zbus::interface(name = "org.kde.kwin.Script")]
    impl FakeScript {
        #[zbus(name = "run")]
        fn run(&self) {
            // what the script does when it starts: report the active window
            std::thread::spawn(|| {
                let conn = zbus::blocking::Connection::session().unwrap();
                let pid = std::process::id().to_string();
                conn.call_method(Some(BUS_NAME), OBJECT_PATH, Some(INTERFACE), "Update", &("org.kde.konsole", pid.as_str())).unwrap();
            });
        }
    }

    /// Needs a D-Bus session bus: `dbus-run-session -- cargo test kwin_round_trip -- --ignored`.
    #[test]
    #[ignore]
    fn kwin_round_trip() {
        let _kwin = zbus::blocking::connection::Builder::session()
            .unwrap()
            .name("org.kde.KWin")
            .unwrap()
            .serve_at("/Scripting", FakeScripting)
            .unwrap()
            .serve_at("/Scripting/Script7", FakeScript)
            .unwrap()
            .build()
            .unwrap();
        start_kwin().unwrap();
        let _ = SOURCE.set(Some(Source::KWin));
        for _ in 0..50 {
            if KWIN_ACTIVE.lock().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let app = active_app().unwrap().expect("the script reported a window");
        assert_eq!(app.pid, Some(std::process::id()));
        let comm = std::fs::read_to_string("/proc/self/comm").unwrap();
        assert_eq!(app.name, comm.trim());
        stop();
    }

    /// Needs a running sway with a focused window: `SWAYSOCK=... cargo test sway_live -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn sway_live() {
        let app = sway_active().expect("a focused window");
        println!("sway active window: {app:?}");
        assert!(!app.name.is_empty());
    }

    #[test]
    fn kwin_script_supports_both_plasma_versions() {
        let s = kwin_script();
        assert!(s.contains("workspace.windowActivated || workspace.clientActivated"));
        assert!(s.contains("\"io.github.expandercraft.ActiveWindow\", \"Update\""));
    }
}
