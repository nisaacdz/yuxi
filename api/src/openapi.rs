use utoipa::OpenApi;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Yuxi API",
        version = "0.1.0",
        description = "Rust backend API for high performance typing web applications",
        license(name = "MIT"),
    ),
    paths(
        crate::routers::auth::login_post,
        crate::routers::auth::register_post,
        crate::routers::auth::me_get,
        crate::routers::auth::forgot_password_post,
        crate::routers::auth::reset_password_post,
        crate::routers::auth::google_auth_post,
        crate::routers::user::users_post,
        crate::routers::user::users_id_get,
        crate::routers::user::current_user_update,
        crate::routers::tournament::tournaments_post,
        crate::routers::tournament::tournaments_get,
        crate::routers::tournament::tournaments_id_get,
    ),
    components(
        schemas(
            crate::ApiResponse<models::schemas::user::LoginSchema>,
            crate::ApiResponse<models::schemas::user::UserSchema>,
            crate::ApiResponse<models::schemas::user::AuthSchema>,
            crate::ApiResponse<models::schemas::tournament::TournamentSchema>,
            crate::ApiResponse<models::schemas::pagination::PaginatedData<models::schemas::tournament::Tournament>>,
            models::schemas::user::UserSchema,
            models::schemas::user::LoginSchema,
            models::schemas::user::TokensSchema,
            models::schemas::user::AuthSchema,
            models::schemas::user::TournamentRoomUserProfile,
            models::schemas::user::TournamentRoomMember,
            models::schemas::tournament::TournamentSchema,
            models::schemas::tournament::TournamentListSchema,
            models::schemas::tournament::TournamentSession,
            models::schemas::tournament::Tournament,
            models::schemas::tournament::TournamentLiveData,
            models::schemas::typing::TextOptions,
            models::schemas::typing::TournamentStatus,
            models::schemas::typing::TypingSessionSchema,
            models::schemas::pagination::PaginatedData<models::schemas::tournament::Tournament>,
            models::schemas::pagination::ListSchema<models::schemas::tournament::TournamentSchema>,
            models::params::user::CreateUserParams,
            models::params::user::LoginUserParams,
            models::params::user::UpdateUserParams,
            models::params::user::ForgotPasswordBody,
            models::params::user::ResetPasswordBody,
            models::params::user::AuthCodeParams,
            models::params::tournament::CreateTournamentParams,
            models::params::tournament::UpdateTournamentParams,
            models::domains::sea_orm_active_enums::TournamentPrivacy,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "Authentication endpoints"),
        (name = "users", description = "User management endpoints"),
        (name = "tournaments", description = "Tournament management endpoints"),
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some("JWT token authentication"))
                        .build(),
                ),
            );
        }
    }
}
