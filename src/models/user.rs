use chrono::NaiveDateTime;

use validator::Validate;

use models::{AuthenticationToken, UserId};
use schema::users;

#[derive(Debug, Queryable, Clone)]
pub struct User {
    pub id: UserId,
    pub name: String,
    pub authentication_token: AuthenticationToken,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl Default for User {
    fn default() -> Self {
        Self {
            id: UserId::generate(),
            name: String::default(),
            authentication_token: AuthenticationToken::default(),
            created_at: ::chrono::Utc::now().naive_utc(),
            updated_at: ::chrono::Utc::now().naive_utc(),
        }
    }
}

#[derive(Debug, Insertable, Validate, Clone)]
#[table_name = "users"]
pub struct NewUser {
    pub id: UserId,
    #[validate(length(min = "1", max = "40", message = "Name must not be empty "))]
    pub name: String,
    #[validate]
    pub authentication_token: AuthenticationToken,
}

impl Default for NewUser {
    fn default() -> Self {
        Self {
            id: UserId::generate(),
            name: "Anonymous".to_string(),
            authentication_token: AuthenticationToken::default(),
        }
    }
}

/// Payload for updating users
#[derive(Debug, Insertable, Validate, AsChangeset, Clone, Default)]
#[table_name = "users"]
pub struct UpdateUser {
    #[validate(length(min = "1", max = "40", message = "Name must not be empty "))]
    pub name: Option<String>,
    #[validate]
    pub authentication_token: Option<AuthenticationToken>,
}
