use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisterUserRequest {
    #[schema(example = "budi.santoso@example.com", required = true)]
    pub email: Option<String>,
    #[schema(example = "Budi Santoso", required = true)]
    pub full_name: Option<String>,
    #[schema(example = "secret123", required = true)]
    pub password: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    #[schema(example = "budi.santoso@example.com", required = true)]
    pub email: Option<String>,
    #[schema(example = "secret123", required = true)]
    pub password: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RefreshTokenRequest {
    #[schema(example = "0123456789abcdef...", required = true)]
    pub refresh_token: Option<String>,
}
