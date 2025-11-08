# OpenAPI Documentation

This project includes comprehensive OpenAPI 3.0 documentation for all REST API endpoints.

## Accessing the Documentation

### Swagger UI (Interactive Documentation)

Once the server is running, you can access the interactive Swagger UI at:

```
http://localhost:8000/api-docs
```

The Swagger UI provides:
- Interactive API exploration
- Try-it-out functionality for all endpoints
- Request/response examples
- Schema definitions
- Authentication testing with JWT tokens

### OpenAPI JSON Specification

The raw OpenAPI specification in JSON format is available at:

```
http://localhost:8000/api-docs/openapi.json
```

This can be used to:
- Generate client SDKs in various languages
- Import into API testing tools (Postman, Insomnia, etc.)
- Generate documentation in other formats
- Validate API compliance

## API Structure

### Authentication Endpoints (`/api/v1/auth`)

- `POST /api/v1/auth/register` - Register a new user
- `POST /api/v1/auth/login` - Login with email and password
- `GET /api/v1/auth/me` - Get current user information (requires authentication)
- `POST /api/v1/auth/forgot-password` - Request password reset OTP
- `POST /api/v1/auth/reset-password` - Reset password with OTP
- `POST /api/v1/auth/google` - Authenticate with Google OAuth

### User Endpoints (`/api/v1/users`)

- `POST /api/v1/users` - Create a new user
- `GET /api/v1/users/{id}` - Get user by ID
- `GET /api/v1/users/me` - Get current user (requires authentication)
- `PATCH /api/v1/users/me` - Update current user (requires authentication)

### Tournament Endpoints (`/api/v1/tournaments`)

- `GET /api/v1/tournaments` - List tournaments with pagination and filters
- `POST /api/v1/tournaments` - Create a new tournament (requires authentication)
- `GET /api/v1/tournaments/{id}` - Get tournament by ID

## Authentication

Most endpoints require JWT authentication. To authenticate:

1. Login or register to obtain an access token
2. In Swagger UI, click the "Authorize" button
3. Enter your token in the format: `Bearer YOUR_TOKEN_HERE`
4. Click "Authorize" to save

For programmatic access, include the token in the `Authorization` header:

```
Authorization: Bearer YOUR_TOKEN_HERE
```

## Implementation Details

The OpenAPI documentation is implemented using:

- **utoipa** - Rust OpenAPI code generation
- **utoipa-swagger-ui** - Swagger UI integration for Axum
- Compile-time schema generation from Rust types
- Type-safe request/response definitions

All schemas are automatically derived from the actual Rust data structures, ensuring the documentation always matches the implementation.

## Generating Client SDKs

You can use the OpenAPI specification to generate client libraries for various languages:

### Using OpenAPI Generator

```bash
# Install openapi-generator-cli
npm install -g @openapitools/openapi-generator-cli

# Generate TypeScript client
openapi-generator-cli generate \
  -i http://localhost:8000/api-docs/openapi.json \
  -g typescript-axios \
  -o ./generated/typescript-client

# Generate Python client
openapi-generator-cli generate \
  -i http://localhost:8000/api-docs/openapi.json \
  -g python \
  -o ./generated/python-client
```

### Using Swagger Codegen

```bash
# Generate Java client
swagger-codegen generate \
  -i http://localhost:8000/api-docs/openapi.json \
  -l java \
  -o ./generated/java-client
```

## Development

When adding new endpoints:

1. Add `#[utoipa::path(...)]` attribute to handler functions
2. Include request/response types in the `paths()` macro in `api/src/openapi.rs`
3. Ensure all request/response types derive `ToSchema`
4. Add appropriate tags and descriptions

Example:

```rust
#[utoipa::path(
    post,
    path = "/api/v1/example",
    tag = "examples",
    request_body = ExampleRequest,
    responses(
        (status = 200, description = "Success", body = ApiResponse<ExampleResponse>),
        (status = 400, description = "Bad request"),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn example_handler(
    State(state): State<AppState>,
    Json(body): Json<ExampleRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Handler implementation
}
```

## Maintenance

The OpenAPI documentation is automatically updated whenever:
- New endpoints are added
- Request/response schemas change
- API paths are modified

No manual documentation updates are needed - the spec is generated from the code at compile time.
