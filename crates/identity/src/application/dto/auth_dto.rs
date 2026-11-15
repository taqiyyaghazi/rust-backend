use crate::application::dto::UserDto;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponseDto {
    pub user: UserDto,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenPairDto {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}
