use std::time::SystemTime;

use validator::Validate;

use models::{AuthenticationToken, UserId};
use schema::users;

#[derive(Debug, Deserialize, Serialize, Queryable, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: UserId,
    pub name: String,
    pub authentication_token: AuthenticationToken,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

#[derive(Debug, Deserialize, Insertable, Validate, Clone)]
#[serde(rename_all = "camelCase")]
#[table_name = "users"]
pub struct NewUser {
    pub id: UserId,
    #[validate(length(min = "1", max = "40", message = "Name must not be empty "))]
    pub name: String,
    #[validate]
    pub authentication_token: AuthenticationToken,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

/// Payload for updating users
#[derive(Debug, Deserialize, Insertable, Validate, AsChangeset, Clone)]
#[table_name = "users"]
pub struct UpdateUser {
    #[validate(length(min = "1", max = "40", message = "Name must not be empty "))]
    pub name: Option<String>,
    #[validate]
    pub authentication_token: Option<AuthenticationToken>,
}
