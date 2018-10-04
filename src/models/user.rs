use validator::Validate;

use models::Password;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Validate)]
#[serde(rename_all = "camelCase")]
pub struct NewUser {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    #[validate(length(min = "1", message = "First name must not be empty"))]
    pub first_name: String,
    #[validate(length(min = "1", message = "Last name must not be empty"))]
    pub last_name: String,
    #[validate]
    pub password: Password,
}

impl NewUser {
    pub fn new(email: String, first_name: String, last_name: String, password: Password) -> Self {
        Self {
            email,
            first_name,
            last_name,
            password,
        }
    }
}
