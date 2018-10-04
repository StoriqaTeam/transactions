#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserId(String);

impl UserId {
    pub fn new(user_id: String) -> Self {
        UserId(user_id)
    }
}

impl UserId {
    pub fn inner(&self) -> &str {
        &self.0
    }
}
