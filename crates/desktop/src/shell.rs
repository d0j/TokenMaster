use std::{cell::RefCell, rc::Rc};

use anyhow::Context;
use slint::BackendSelector;

pub const PRODUCTION_RENDERER: &str = "winit-software";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopLifecycleIntent {
    Show,
    Hide,
    OpenCompact,
    OpenDashboard,
    Quit,
}

impl DesktopLifecycleIntent {
    pub const ALL: [Self; 5] = [
        Self::Show,
        Self::Hide,
        Self::OpenCompact,
        Self::OpenDashboard,
        Self::Quit,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopLifecycleIntentAdmission {
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopTrayAvailability {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopCloseEffect {
    HideToTray,
    Quit,
}

impl DesktopTrayAvailability {
    #[must_use]
    pub const fn close_effect(self) -> DesktopCloseEffect {
        match self {
            Self::Available => DesktopCloseEffect::HideToTray,
            Self::Unavailable => DesktopCloseEffect::Quit,
        }
    }
}

pub trait DesktopLifecycleIntentSink {
    fn submit(&self, intent: DesktopLifecycleIntent) -> DesktopLifecycleIntentAdmission;
}

#[derive(Default)]
pub struct DesktopLifecycleIntentRouter {
    sink: RefCell<Option<Rc<dyn DesktopLifecycleIntentSink>>>,
}

impl DesktopLifecycleIntentRouter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sink: RefCell::new(None),
        }
    }

    pub fn install(
        &self,
        sink: Rc<dyn DesktopLifecycleIntentSink>,
    ) -> Result<(), DesktopLifecycleIntentRouterError> {
        let mut slot = self
            .sink
            .try_borrow_mut()
            .map_err(|_| DesktopLifecycleIntentRouterError)?;
        if slot.is_some() {
            return Err(DesktopLifecycleIntentRouterError);
        }
        *slot = Some(sink);
        Ok(())
    }
}

impl DesktopLifecycleIntentSink for DesktopLifecycleIntentRouter {
    fn submit(&self, intent: DesktopLifecycleIntent) -> DesktopLifecycleIntentAdmission {
        let Ok(slot) = self.sink.try_borrow() else {
            return DesktopLifecycleIntentAdmission::Rejected;
        };
        slot.as_ref()
            .map_or(DesktopLifecycleIntentAdmission::Rejected, |sink| {
                sink.submit(intent)
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopLifecycleIntentRouterError;

pub fn select_production_renderer() -> anyhow::Result<()> {
    BackendSelector::new()
        .backend_name(PRODUCTION_RENDERER.to_owned())
        .select()
        .context("select production software renderer")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{DesktopCloseEffect, DesktopTrayAvailability};

    #[test]
    fn available_tray_hides_the_window_on_close() {
        assert_eq!(
            DesktopTrayAvailability::Available.close_effect(),
            DesktopCloseEffect::HideToTray
        );
    }

    #[test]
    fn unavailable_tray_quits_instead_of_stranding_a_hidden_process() {
        assert_eq!(
            DesktopTrayAvailability::Unavailable.close_effect(),
            DesktopCloseEffect::Quit
        );
    }
}
