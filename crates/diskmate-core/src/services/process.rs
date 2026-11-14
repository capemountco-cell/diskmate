//! The Process service: which apps are using memory, and quitting one.
//!
//! Quitting is DiskMate's only process action, and it goes through here and
//! nowhere else, the way removal goes through `CleanupService`. The rules:
//!
//! * It is a quit request, the same one the app's own Quit menu item sends. The
//!   app can save, ask about unsaved work, or decline. Nothing is ever killed.
//! * It is offered only for ordinary apps with a Dock icon. Background agents,
//!   daemons and command-line processes are listed for information only.
//! * The caller names the app it means as well as its process id. If that pid
//!   now belongs to something else, or the app has already quit, the request
//!   is refused, so a stale menu can never quit the wrong thing.
//! * DiskMate never quits itself through this path.

use crate::error::{CoreError, Result};
use crate::model::ProcessMemory;
use crate::platform::{self, AppQuitter, SystemProbe, SystemQuitter};

/// How many apps are examined when checking a quit request against what is
/// running now. Generous, so an app that has just shrunk is still found.
const VERIFY_WINDOW: usize = 200;

pub struct ProcessService {
    probe: &'static dyn SystemProbe,
    quitter: Box<dyn AppQuitter>,
    own_pid: i32,
}

impl Default for ProcessService {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessService {
    pub fn new() -> Self {
        Self::with_parts(platform::probe(), Box::new(SystemQuitter))
    }

    /// For tests: a fake probe and a recording quitter.
    pub fn with_parts(probe: &'static dyn SystemProbe, quitter: Box<dyn AppQuitter>) -> Self {
        Self {
            probe,
            quitter,
            own_pid: std::process::id() as i32,
        }
    }

    /// The apps using the most memory, largest first. DiskMate itself is never
    /// offered a Quit here: closing it is what its own Quit is for.
    ///
    /// Nor is Finder: macOS opens it again the moment it quits, so a Quit would
    /// free nothing and only make the desktop blink.
    pub fn top_apps(&self, limit: usize) -> Result<Vec<ProcessMemory>> {
        let mut apps = self.probe.top_apps(limit)?;
        for app in &mut apps {
            if app.pid == self.own_pid || reopened_by_macos(app) {
                app.can_quit = false;
            }
        }
        Ok(apps)
    }

    /// Ask the app `name`, whose main process is `pid`, to quit.
    pub fn quit_app(&self, pid: i32, name: &str) -> Result<()> {
        if pid == self.own_pid {
            return Err(CoreError::invalid(
                "Mr Clean does not quit itself this way.",
            ));
        }
        let current = self
            .probe
            .top_apps(VERIFY_WINDOW)?
            .into_iter()
            .find(|app| app.pid == pid);
        let Some(app) = current else {
            return Err(CoreError::invalid(format!("{name} is no longer running.")));
        };
        if app.name != name {
            return Err(CoreError::invalid(format!(
                "{name} is no longer running; that process is now {}.",
                app.name
            )));
        }
        if reopened_by_macos(&app) {
            return Err(CoreError::invalid(format!(
                "macOS opens {name} again as soon as it quits, so quitting it frees nothing."
            )));
        }
        if !app.can_quit {
            return Err(CoreError::invalid(format!(
                "{name} runs in the background, so Mr Clean does not offer to quit it."
            )));
        }
        tracing::info!(
            pid,
            name,
            quitter = self.quitter.name(),
            "asking app to quit"
        );
        self.quitter
            .request_quit(pid)
            .map_err(|reason| CoreError::invalid(format!("Could not ask {name} to quit: {reason}")))
    }
}

/// Apps macOS reopens by itself the moment they quit.
fn reopened_by_macos(app: &ProcessMemory) -> bool {
    app.bundle_path.as_deref() == Some("/System/Library/CoreServices/Finder.app")
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::model::{MemorySnapshot, StartupItem, VolumeInfo};

    struct FakeProbe(Vec<ProcessMemory>);

    impl SystemProbe for FakeProbe {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn volumes(&self) -> Result<Vec<VolumeInfo>> {
            Ok(Vec::new())
        }
        fn memory(&self) -> Result<MemorySnapshot> {
            Err(CoreError::Unsupported { feature: "memory" })
        }
        fn top_apps(&self, limit: usize) -> Result<Vec<ProcessMemory>> {
            Ok(self.0.iter().take(limit).cloned().collect())
        }
        fn startup_items(&self) -> Result<Vec<StartupItem>> {
            Ok(Vec::new())
        }
        fn skip_mount_points(&self, _root: &Path) -> Vec<PathBuf> {
            Vec::new()
        }
    }

    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<i32>>>);

    impl AppQuitter for Recorder {
        fn request_quit(&self, pid: i32) -> std::result::Result<(), String> {
            self.0.lock().unwrap().push(pid);
            Ok(())
        }
        fn name(&self) -> &'static str {
            "recorder"
        }
    }

    fn app(pid: i32, name: &str, can_quit: bool) -> ProcessMemory {
        ProcessMemory {
            pid,
            name: name.to_string(),
            resident_bytes: 1_000,
            instance_count: 1,
            bundle_path: Some(format!("/Applications/{name}.app")),
            can_quit,
        }
    }

    fn service(apps: Vec<ProcessMemory>) -> (ProcessService, Recorder) {
        let recorder = Recorder::default();
        let probe: &'static FakeProbe = Box::leak(Box::new(FakeProbe(apps)));
        (
            ProcessService::with_parts(probe, Box::new(recorder.clone())),
            recorder,
        )
    }

    #[test]
    fn quits_the_named_app() {
        let (service, recorder) = service(vec![app(42, "Editor", true)]);
        service.quit_app(42, "Editor").unwrap();
        assert_eq!(*recorder.0.lock().unwrap(), vec![42]);
    }

    #[test]
    fn refuses_when_the_pid_now_belongs_to_another_app() {
        let (service, recorder) = service(vec![app(42, "Terminal", true)]);
        let error = service.quit_app(42, "Editor").unwrap_err().to_string();
        assert!(error.contains("no longer running"), "{error}");
        assert!(recorder.0.lock().unwrap().is_empty());
    }

    #[test]
    fn refuses_an_app_that_has_gone() {
        let (service, recorder) = service(vec![]);
        assert!(service.quit_app(42, "Editor").is_err());
        assert!(recorder.0.lock().unwrap().is_empty());
    }

    #[test]
    fn refuses_background_processes() {
        let (service, recorder) = service(vec![app(7, "syncagent", false)]);
        let error = service.quit_app(7, "syncagent").unwrap_err().to_string();
        assert!(error.contains("background"), "{error}");
        assert!(recorder.0.lock().unwrap().is_empty());
    }

    #[test]
    fn finder_is_never_offered_a_quit_because_macos_reopens_it() {
        let mut finder = app(9, "Finder", true);
        finder.bundle_path = Some("/System/Library/CoreServices/Finder.app".into());
        let (service, recorder) = service(vec![finder]);
        assert!(!service.top_apps(5).unwrap()[0].can_quit);
        let error = service.quit_app(9, "Finder").unwrap_err().to_string();
        assert!(error.contains("opens Finder again"), "{error}");
        assert!(recorder.0.lock().unwrap().is_empty());
    }

    #[test]
    fn never_offers_or_performs_quitting_itself() {
        let own = std::process::id() as i32;
        let (service, recorder) = service(vec![app(own, "Mr Clean", true)]);
        assert!(!service.top_apps(5).unwrap()[0].can_quit);
        assert!(service.quit_app(own, "Mr Clean").is_err());
        assert!(recorder.0.lock().unwrap().is_empty());
    }
}
