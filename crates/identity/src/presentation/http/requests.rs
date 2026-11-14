use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RegisterUserRequest {
    #[schema(example = "budi.santoso@example.com")]
    pub email: String,
    #[schema(example = "Budi Santoso")]
    pub full_name: String,
}
