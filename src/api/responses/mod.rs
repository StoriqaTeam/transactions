use std::time::SystemTime;

use models::*;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsersResponse {
    pub id: UserId,
    pub name: String,
    pub authentication_token: AuthenticationToken,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl From<User> for UsersResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            name: user.name,
            authentication_token: user.authentication_token,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}
