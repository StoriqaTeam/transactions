use models::*;

#[derive(Debug, Deserialize, Clone)]
pub struct CreateUserResponse {
    pub data: Option<CreateUser>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateUser {
    pub create_user: User,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MeResponse {
    pub data: Option<Me>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Me {
    pub me: User,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GetJWTResponse {
    pub data: Option<GetJWTByEmail>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GetJWTByEmail {
    #[serde(rename = "getJWTByEmail")]
    pub get_jwt_by_email: Token,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GetJWTByProviderResponse {
    pub data: Option<GetJWTByProvider>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GetJWTByProvider {
    #[serde(rename = "getJWTByProvider")]
    pub get_jwt_by_provider: Token,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Token {
    pub token: StoriqaJWT,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GraphQLError {
    pub message: String,
    pub path: Option<Vec<String>>,
    pub data: Option<GraphQLErrorData>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GraphQLErrorData {
    pub code: usize,
    pub details: GraphQLErrorDataDetails,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GraphQLErrorDataDetails {
    status: String,
    code: String,
    description: String,
    message: String,
    payload: String,
}
