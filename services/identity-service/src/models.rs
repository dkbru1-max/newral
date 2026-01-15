use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: Option<String>,
    pub password: Option<String>,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub status: &'static str,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: Option<String>,
    pub password: Option<String>,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub status: &'static str,
    pub token: &'static str,
}

#[derive(Deserialize)]
pub struct DeviceRegisterRequest {
    pub user_id: String,
    pub device_id: String,
}

#[derive(Serialize)]
pub struct DeviceRegisterResponse {
    pub status: &'static str,
    pub device_count: usize,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub code: &'static str,
    pub message: &'static str,
}
