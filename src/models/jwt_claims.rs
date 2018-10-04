#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JWTClaims {
    pub user_id: usize,
    pub exp: u64,
    pub provider: String,
}
