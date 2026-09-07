use axum::{Json, extract::State, http::StatusCode};
use crate::{api::api_state::{ApiResponse, ApiState}, enums::{jwt::JwtType, role::RoleType}, models::{login::{Login, RefreshRequest, Token}, user::User}, utils::{self, password::hash_password}};

/// Map a stored role string back to a `RoleType` (defaults to `User`).
fn role_from_str(role: &str) -> RoleType {
    match role {
        "admin" => RoleType::Admin,
        _ => RoleType::User,
    }
}


pub async fn create_user(
    State(state): State<ApiState>,
    Json(mut user): Json<User>,
) -> (StatusCode, Json<ApiResponse<User>>) {

    let hashed_password = match  hash_password(&user.password_hash) {
        Ok(passwd) => passwd,
        Err(e) => {
            return  (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())));
        }
    };

    user.password_hash = hashed_password;

    match state.storage.user.create(&user) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(ApiResponse::success(user, "successfully created")),
        ),

        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch users: {}", e),
            ))
        )
    }
}
// delete user 

// login user 

pub async fn login(
    State(state): State<ApiState>,
    Json(data): Json<Login>,
)-> (StatusCode, Json<ApiResponse<Token>>) {
    let user = match state.storage.user.get(&data.username) {
        Ok(Some(user)) => user,
        _ => return (StatusCode::BAD_REQUEST, Json(ApiResponse::error(StatusCode::BAD_REQUEST, "client not found")))
        
    };

    if !utils::password::verify(&data.password, &user.password_hash) {
        return (StatusCode::BAD_REQUEST, Json(ApiResponse::error(StatusCode::BAD_REQUEST, "wrong password")));
    }

    let token = match state.jwt_service.generate(user.username, RoleType::Admin) {
        Ok(token) => token,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(ApiResponse::error(StatusCode::BAD_REQUEST, e.to_string())));
        }
    };

    (StatusCode::OK, Json(ApiResponse::success(token, "successfully created")))


}

// refresh access token
pub async fn refresh_token(
    State(state): State<ApiState>,
    Json(data): Json<RefreshRequest>,
) -> (StatusCode, Json<ApiResponse<Token>>) {
    let claims = match state.jwt_service.parse(&data.refresh_token) {
        Ok(claims) => claims,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::error(StatusCode::UNAUTHORIZED, "invalid refresh token")),
            );
        }
    };

    if claims.token_type != JwtType::RefreshToken.as_str() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::error(StatusCode::UNAUTHORIZED, "not a refresh token")),
        );
    }

    match state.jwt_service.generate(claims.sub, role_from_str(&claims.role)) {
        Ok(token) => (StatusCode::OK, Json(ApiResponse::success(token, "token refreshed"))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
        ),
    }
}

// logout user

// delete user

// get users
pub async fn get_all_users(
    State(state): State<ApiState>,
) -> (StatusCode, Json<ApiResponse<Vec<User>>>) {
    match state.storage.user.get_all() {
        Ok(users) => (
            StatusCode::OK,
            Json(ApiResponse::success(users, "Fetched all users successfully")),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch users: {}", e),
            )),
        ),
    }
}
