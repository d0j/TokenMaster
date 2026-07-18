use tokenmaster_desktop::DesktopSnapshotEpoch;
use tokenmaster_desktop::presentation::{
    DesktopApplyOutcome, DesktopProjection, DesktopRouteKey, DesktopRouteState,
    DesktopSelectionError, DesktopState,
};
use tokenmaster_product::{
    ProductAttemptGeneration, ProductReducer, ProductRoute, ProductRouteState,
};
use tokenmaster_query::QueryErrorCode;

#[test]
fn initial_product_snapshot_maps_to_exact_bounded_routes() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let projection = DesktopProjection::from_snapshot(&snapshot, DesktopRouteKey::Dashboard);

    assert_eq!(projection.generation(), snapshot.generation());
    assert_eq!(projection.routes().len(), 11);
    assert_eq!(projection.selected(), DesktopRouteKey::Dashboard);

    for (index, route) in ProductRoute::ALL.into_iter().enumerate() {
        let projected = &projection.routes()[index];
        let source = snapshot.route(route);
        assert_eq!(projected.route(), route);
        assert_eq!(projected.key().product_route(), route);
        assert_eq!(projected.state(), DesktopRouteState::from(source.state()));
        assert_eq!(
            projected.reason_codes().iter().collect::<Vec<_>>(),
            source
                .reasons()
                .iter()
                .map(|reason| reason.stable_code())
                .collect::<Vec<_>>()
        );
        assert!(projected.reason_codes().len() <= 11);
    }
}

#[test]
fn route_keys_and_labels_are_stable_and_complete() {
    let expected = [
        ("dashboard", "route.dashboard"),
        ("history", "route.history"),
        ("sessions", "route.sessions"),
        ("models", "route.models"),
        ("projects", "route.projects"),
        ("activity", "route.activity"),
        ("data_health", "route.data_health"),
        ("notifications", "route.notifications"),
        ("settings", "route.settings"),
        ("help_about", "route.help_about"),
        ("compact_widget", "route.compact_widget"),
    ];

    for (key, (stable_key, label_key)) in DesktopRouteKey::ALL.into_iter().zip(expected) {
        assert_eq!(key.stable_key(), stable_key);
        assert_eq!(key.label_key(), label_key);
        assert_eq!(DesktopRouteKey::from_stable_key(stable_key), Some(key));
    }
}

#[test]
fn settings_and_help_are_ready_without_archive() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let projection = DesktopProjection::from_snapshot(&snapshot, DesktopRouteKey::Dashboard);

    assert_eq!(
        projection.route(DesktopRouteKey::Settings).state(),
        DesktopRouteState::Ready
    );
    assert_eq!(
        projection.route(DesktopRouteKey::HelpAbout).state(),
        DesktopRouteState::Ready
    );
    assert_eq!(
        snapshot.route(ProductRoute::Settings).state(),
        ProductRouteState::Ready
    );
    assert_eq!(
        projection
            .route(DesktopRouteKey::Dashboard)
            .reason_codes()
            .iter()
            .collect::<Vec<_>>(),
        vec!["data_status_unavailable"]
    );
}

#[test]
fn unknown_selection_is_rejected_without_changing_selection() {
    let reducer = ProductReducer::new();
    let snapshot = reducer.snapshot();
    let mut projection = DesktopProjection::from_snapshot(&snapshot, DesktopRouteKey::Dashboard);

    assert_eq!(
        projection.select_stable_key("not-a-route"),
        Err(DesktopSelectionError)
    );
    assert_eq!(projection.selected(), DesktopRouteKey::Dashboard);
    projection
        .select_stable_key("settings")
        .expect("known route must select");
    assert_eq!(projection.selected(), DesktopRouteKey::Settings);
}

#[test]
fn state_accepts_only_newer_product_generations_and_retains_selection() {
    let mut reducer = ProductReducer::new();
    let initial = reducer.snapshot();
    let mut state = DesktopState::new(&initial, DesktopRouteKey::Dashboard);
    state
        .select_stable_key("settings")
        .expect("known route must select");

    reducer
        .fail_data_status(
            ProductAttemptGeneration::new(1).expect("nonzero attempt"),
            QueryErrorCode::DeadlineExceeded,
        )
        .expect("new product generation");
    let newer = reducer.snapshot();

    assert_eq!(state.apply_snapshot(&newer), DesktopApplyOutcome::Accepted);
    assert_eq!(state.projection().generation(), newer.generation());
    assert_eq!(state.projection().selected(), DesktopRouteKey::Settings);
    assert_eq!(
        state.apply_snapshot(&newer),
        DesktopApplyOutcome::IgnoredNotNewer
    );
    assert_eq!(
        state.apply_snapshot(&initial),
        DesktopApplyOutcome::IgnoredNotNewer
    );
    assert_eq!(state.projection().generation(), newer.generation());
    assert_eq!(state.projection().selected(), DesktopRouteKey::Settings);
}

#[test]
fn higher_snapshot_epoch_accepts_restarted_generation_and_rejects_old_backend() {
    let mut reducer = ProductReducer::new();
    let initial = reducer.snapshot();
    reducer
        .fail_data_status(
            ProductAttemptGeneration::new(1).expect("nonzero attempt"),
            QueryErrorCode::DeadlineExceeded,
        )
        .expect("new product generation");
    let newer = reducer.snapshot();
    let epoch_one = DesktopSnapshotEpoch::new(1).expect("epoch one");
    let epoch_two = DesktopSnapshotEpoch::new(2).expect("epoch two");
    let mut state = DesktopState::new(&initial, DesktopRouteKey::Dashboard);
    state
        .select_stable_key("settings")
        .expect("known route must select");

    assert_eq!(
        state.apply_snapshot_for_epoch(epoch_one, &newer),
        DesktopApplyOutcome::Accepted
    );
    assert_eq!(state.snapshot_epoch(), Some(epoch_one));
    assert_eq!(state.projection().generation(), newer.generation());
    assert_eq!(
        state.apply_snapshot_for_epoch(epoch_one, &newer),
        DesktopApplyOutcome::IgnoredNotNewer
    );

    assert_eq!(
        state.apply_snapshot_for_epoch(epoch_two, &initial),
        DesktopApplyOutcome::Accepted
    );
    assert_eq!(state.snapshot_epoch(), Some(epoch_two));
    assert_eq!(state.projection().generation(), initial.generation());
    assert_eq!(state.projection().selected(), DesktopRouteKey::Settings);

    assert_eq!(
        state.apply_snapshot_for_epoch(epoch_one, &newer),
        DesktopApplyOutcome::IgnoredNotNewer
    );
    assert_eq!(state.snapshot_epoch(), Some(epoch_two));
    assert_eq!(state.projection().generation(), initial.generation());
}
