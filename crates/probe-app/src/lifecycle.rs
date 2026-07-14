#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Show,
    Hide,
    CloseRequested,
    Quit,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Visibility {
    #[default]
    Visible,
    Hidden,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lifecycle {
    visibility: Visibility,
    quit_requested: bool,
    window_generation: u64,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self {
            visibility: Visibility::Visible,
            quit_requested: false,
            window_generation: 1,
        }
    }
}

impl Lifecycle {
    pub fn apply(&mut self, action: Action) -> Visibility {
        match action {
            Action::Show => self.visibility = Visibility::Visible,
            Action::Hide | Action::CloseRequested => self.visibility = Visibility::Hidden,
            Action::Quit => self.quit_requested = true,
        }
        self.visibility
    }

    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    pub fn window_generation(&self) -> u64 {
        self.window_generation
    }
}
