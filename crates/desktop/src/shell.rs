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
