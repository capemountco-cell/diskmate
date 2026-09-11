//! The Startup service.
//!
//! Read-only in Milestone 01, and structured so that it stays read-only until
//! there is a deliberate, reviewed reason to change that. Enabling, disabling
//! or removing a launch item is not implemented anywhere in this crate.
//!
//! Impact classification is not implemented either. Until DiskMate can measure
//! launch cost defensibly, every item reports `Unmeasured`; the UI shows that
//! honestly rather than printing a made-up "High impact" badge.

use crate::error::Result;
use crate::model::{StartupImpact, StartupItem, StartupScope};
use crate::platform::{self, SystemProbe};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupReport {
    pub items: Vec<StartupItem>,
    pub user_count: usize,
    pub system_count: usize,
    pub os_count: usize,
    /// True while impact measurement is unimplemented, so the UI can say so
    /// instead of rendering an empty column.
    pub impact_unavailable: bool,
}

pub struct StartupService {
    probe: &'static dyn SystemProbe,
}

impl Default for StartupService {
    fn default() -> Self {
        Self::new()
    }
}

impl StartupService {
    pub fn new() -> Self {
        Self { probe: platform::probe() }
    }

    pub fn report(&self) -> Result<StartupReport> {
        let items = self.probe.startup_items()?;
        Ok(summarise(items))
    }
}

pub(crate) fn summarise(items: Vec<StartupItem>) -> StartupReport {
    let user_count = items.iter().filter(|i| i.scope == StartupScope::User).count();
    let system_count = items.iter().filter(|i| i.scope == StartupScope::System).count();
    let os_count = items.iter().filter(|i| i.scope == StartupScope::Os).count();
    let impact_unavailable = items.iter().all(|i| i.impact == StartupImpact::Unmeasured);
    StartupReport { items, user_count, system_count, os_count, impact_unavailable }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{StartupSource};

    fn item(scope: StartupScope) -> StartupItem {
        StartupItem {
            id: "com.example.helper".into(),
            name: "Example Helper".into(),
            source: StartupSource::LaunchAgent,
            scope,
            config_path: "/Library/LaunchAgents/com.example.helper.plist".into(),
            program: Some("/Applications/Example.app/Contents/MacOS/Helper".into()),
            runs_at_load: Some(true),
            disabled: None,
            impact: StartupImpact::Unmeasured,
            is_os_provided: scope == StartupScope::Os,
        }
    }

    #[test]
    fn scopes_are_counted_separately() {
        let report = summarise(vec![
            item(StartupScope::User),
            item(StartupScope::System),
            item(StartupScope::System),
            item(StartupScope::Os),
        ]);
        assert_eq!(report.user_count, 1);
        assert_eq!(report.system_count, 2);
        assert_eq!(report.os_count, 1);
    }

    #[test]
    fn impact_is_reported_as_unavailable_rather_than_guessed() {
        let report = summarise(vec![item(StartupScope::User)]);
        assert!(report.impact_unavailable);
        assert!(report.items.iter().all(|i| i.impact == StartupImpact::Unmeasured));
    }
}
