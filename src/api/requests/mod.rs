use models::*;

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Ios,
    Android,
    Web,
    Other,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PostSessionsRequest {
    pub email: String,
    pub password: Password,
    pub device_type: DeviceType,
    pub device_os: Option<String>,
    pub device_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PostSessionsOauthRequest {
    pub oauth_token: OauthToken,
    pub oauth_provider: Provider,
    pub device_type: DeviceType,
    pub device_os: Option<String>,
    pub device_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PostUsersRequest {
    pub email: String,
    pub password: Password,
    pub first_name: String,
    pub last_name: String,
    pub device_type: DeviceType,
    pub device_os: Option<String>,
    pub device_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PostUsersConfirmEmailRequest {
    pub email_confirm_token: EmailConfirmToken,
}
