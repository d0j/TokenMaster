use rusqlite::Connection;

pub fn strip_git_schema(connection: &Connection) {
    connection
        .execute_batch(
            "DROP TRIGGER IF EXISTS git_category_no_update;
             DROP TRIGGER IF EXISTS git_day_category_no_update;
             DROP TRIGGER IF EXISTS git_day_no_update;
             DROP TRIGGER IF EXISTS git_installation_state_no_delete;
             DROP TRIGGER IF EXISTS git_warning_no_update;
             DROP INDEX IF EXISTS git_association_repository_activity;
             DROP INDEX IF EXISTS git_day_category_repository_range;
             DROP INDEX IF EXISTS git_day_repository_range;
             DROP INDEX IF EXISTS git_repository_observed;
             DROP TABLE IF EXISTS git_warning;
             DROP TABLE IF EXISTS git_day_category_aggregate;
             DROP TABLE IF EXISTS git_category_aggregate;
             DROP TABLE IF EXISTS git_day_aggregate;
             DROP TABLE IF EXISTS git_activity_association;
             DROP TABLE IF EXISTS git_repository;
             DROP TABLE IF EXISTS git_installation_state;",
        )
        .expect("strip Git v13 schema");
}
