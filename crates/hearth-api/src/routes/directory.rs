use axum::Json;
use axum::extract::State;
use hearth_common::api_types::DirectoryPerson;

use crate::AppState;
use crate::auth::UserIdentity;
use crate::db::UserRow;
use crate::error::AppError;
use crate::repo;

/// Build a `DirectoryPerson` from a `UserRow`, deriving contact info from config.
fn build_directory_person(
    row: UserRow,
    matrix_server_name: Option<&str>,
    nextcloud_url: Option<&str>,
) -> DirectoryPerson {
    let matrix_id = matrix_server_name.map(|server| format!("@{}:{}", row.username, server));
    let nextcloud_url = nextcloud_url.map(|base| {
        let base = base.trim_end_matches('/');
        format!("{}/u/{}", base, row.username)
    });

    DirectoryPerson {
        username: row.username,
        display_name: row.display_name,
        email: row.email,
        groups: row.groups,
        matrix_id,
        nextcloud_url,
        last_seen: row.last_seen,
    }
}

pub async fn list_people(
    _user: UserIdentity,
    State(state): State<AppState>,
) -> Result<Json<Vec<DirectoryPerson>>, AppError> {
    let rows = repo::list_users(&state.pool).await?;
    let nextcloud_url = state
        .services
        .iter()
        .find(|s| s.id == "cloud")
        .map(|s| s.url.as_str());
    let people: Vec<DirectoryPerson> = rows
        .into_iter()
        .map(|row| {
            build_directory_person(row, state.matrix_server_name.as_deref(), nextcloud_url)
        })
        .collect();
    Ok(Json(people))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn fake_user(username: &str) -> UserRow {
        UserRow {
            id: Uuid::new_v4(),
            username: username.to_string(),
            display_name: Some("Alice Smith".to_string()),
            email: Some("alice@example.com".to_string()),
            kanidm_uuid: Some("abc-123".to_string()),
            groups: vec!["hearth-users".to_string(), "hearth-developers".to_string()],
            last_seen: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_build_directory_person_with_all_services() {
        let row = fake_user("alice");
        let person =
            build_directory_person(row, Some("hearth.local"), Some("https://cloud.example.com"));

        assert_eq!(person.username, "alice");
        assert_eq!(person.display_name.as_deref(), Some("Alice Smith"));
        assert_eq!(person.email.as_deref(), Some("alice@example.com"));
        assert_eq!(person.matrix_id.as_deref(), Some("@alice:hearth.local"));
        assert_eq!(
            person.nextcloud_url.as_deref(),
            Some("https://cloud.example.com/u/alice")
        );
        assert_eq!(person.groups.len(), 2);
        assert!(person.last_seen.is_some());
    }

    #[test]
    fn test_build_directory_person_no_services() {
        let row = fake_user("bob");
        let person = build_directory_person(row, None, None);

        assert_eq!(person.username, "bob");
        assert!(person.matrix_id.is_none());
        assert!(person.nextcloud_url.is_none());
    }

    #[test]
    fn test_build_directory_person_partial_matrix_only() {
        let row = fake_user("carol");
        let person = build_directory_person(row, Some("corp.local"), None);

        assert_eq!(person.matrix_id.as_deref(), Some("@carol:corp.local"));
        assert!(person.nextcloud_url.is_none());
    }

    #[test]
    fn test_build_directory_person_partial_nextcloud_only() {
        let row = fake_user("dave");
        let person = build_directory_person(row, None, Some("https://nc.example.com/"));

        assert!(person.matrix_id.is_none());
        assert_eq!(
            person.nextcloud_url.as_deref(),
            Some("https://nc.example.com/u/dave")
        );
    }

    #[test]
    fn test_build_directory_person_no_display_name() {
        let mut row = fake_user("eve");
        row.display_name = None;
        row.email = None;
        let person =
            build_directory_person(row, Some("hearth.local"), Some("https://cloud.example.com"));

        assert_eq!(person.username, "eve");
        assert!(person.display_name.is_none());
        assert!(person.email.is_none());
        assert_eq!(person.matrix_id.as_deref(), Some("@eve:hearth.local"));
    }

    #[test]
    fn test_nextcloud_url_trailing_slash_stripped() {
        let row = fake_user("frank");
        let person = build_directory_person(row, None, Some("https://cloud.example.com///"));

        assert_eq!(
            person.nextcloud_url.as_deref(),
            Some("https://cloud.example.com/u/frank")
        );
    }
}
