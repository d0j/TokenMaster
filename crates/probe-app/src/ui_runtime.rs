use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{ChartPoint, MainWindow, QuotaRow, SessionRow};

pub fn wire_skin_callbacks(window: &MainWindow) {
    let weak = window.as_weak();
    window.on_switch_layout(move |layout_id| {
        if let Some(window) = weak.upgrade()
            && (0..=2).contains(&layout_id)
        {
            window.set_layout_id(layout_id);
        }
    });

    let weak = window.as_weak();
    window.on_switch_theme(move |theme_id| {
        if let Some(window) = weak.upgrade()
            && (0..=2).contains(&theme_id)
        {
            window.set_theme_id(theme_id);
        }
    });

    let weak = window.as_weak();
    window.on_switch_locale(move |locale| {
        let locale = locale.to_string();
        if matches!(locale.as_str(), "en" | "ru")
            && slint::select_bundled_translation(&locale).is_ok()
            && let Some(window) = weak.upgrade()
        {
            window.set_locale_id(locale.into());
        }
    });
}

pub fn seed_probe_models(window: &MainWindow) {
    let quotas = vec![
        quota("burst", "Burst", "42% used", 0.42, "Resets in 23 min"),
        quota("cycle", "Cycle", "68% used", 0.68, "Resets tomorrow"),
        quota("credits", "Credits", "17% used", 0.17, "Rolling balance"),
    ];
    window.set_quota_targets(ModelRc::new(VecModel::from(quotas)));

    let points = (0..120)
        .map(|index| ChartPoint {
            value: ((index % 20) + 1) as f32 / 20.0,
        })
        .collect::<Vec<_>>();
    window.set_chart_points(ModelRc::new(VecModel::from(points)));

    let sessions = (1..=256)
        .map(|id| SessionRow {
            id,
            label: SharedString::from(format!("Session {id}")),
            tokens_label: SharedString::from(format!("{} tokens", id * 100)),
        })
        .collect::<Vec<_>>();
    window.set_session_rows(ModelRc::new(VecModel::from(sessions)));
}

fn quota(id: &str, label: &str, usage: &str, ratio: f32, reset: &str) -> QuotaRow {
    QuotaRow {
        id: id.into(),
        label: label.into(),
        usage_label: usage.into(),
        used_ratio: ratio,
        reset_label: reset.into(),
    }
}
