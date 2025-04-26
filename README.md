# Clean Axum - A Structured Web API Project in Rust

[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Clean Axum is a Rust-based project demonstrating a structured approach to building web APIs using the Axum framework, SeaORM, and a variety of other useful crates. The project emphasizes modularity, clear separation of concerns, and practical implementation of common API features.

## Features

*   **Axum Framework:** Utilizes the high-performance Axum web framework for building APIs.
*   **Clean Structure:** Organizes the project into distinct modules for API logic, application logic, domain models, and utilities, promoting maintainability and scalability.
*   **SeaORM:** Employs SeaORM for type-safe and idiomatic database interactions.
*   **User Authentication:** Implements user registration, login, and logout functionality with session management.
*   **User Management:** Provides endpoints for creating, listing, retrieving, and updating user profiles.
*   **Tournament management:** Provides endpoints to create tournaments, retrieve tournaments by id, and search upcoming tournaments.
*   **Error Handling:** Custom error types and handlers for consistent error management.
*   **Validation:** Uses a custom extractor to enforce API parameter validation.
*   **Middleware:** Session management using `tower-sessions`.
* **Actions manager:** A base for action management (timeout, moderation, etc)
* **Cache:** A customizable cache system.
*   **Database Migrations:** Uses `utils/src/db.rs` to manage database migrations.
*   **Testing:** Includes a basic test suite.

## Modules

*   **`api/`:** Contains the Axum-based API logic, including:
    *   **`routers/`:** Defines API routes (`auth`, `root`, `user`, etc.).
    *   **`models/`:** Defines API input and output schemas.
    *   **`middleware/`**: Handles middleware.
    *   **`error/`:** Contains custom error types and handling logic.
    * **`extractor/`:** Custom extractors.
    *   **`validation/`**: Handle validation.
    * **`action/`:** Action management.
    * **`cache/`:** Cache system.
    *   **`init.rs`:** Initializes the API.
*   **`app/`:** Contains the core application logic, including:
    *   **`persistence/`:** Handles database interactions using SeaORM (`users`, `tournaments`, `text`).
    *   **`error/`:** Custom error handling.
    * **`state/`:** Define the state used by the app.
    * **`config/`:** Handle configuration.
*   **`models/`:** Defines domain models, request parameters, and data schemas.
    *   **`domains/`:** Core data structures (SeaORM entities).
    *   **`params/`:** API request parameters.
    *   **`queries/`:** Define database queries.
    *   **`schemas/`:** Data serialization schemas.
* **`utils/`:** Shared utilities.
    * **`db/`:** Database migration and connection utilities.
    * **`file/`:** File handling utilities.
    * **`testing/`:** utilities for testing.

## Getting Started

1.  **Clone the repository:**
  ```
  git clone https://github.com/nisaacdz/yuxi
  ```

2.  **Install Rust:**

    If you don't have Rust installed, follow the instructions on the official Rust website: [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)

3.  **Set up the database:**

    * Ensure you have a compatible database (e.g., PostgreSQL) installed and running.
    * Configure the `DATABASE_URL` in your environment.

4.  **Run the application in dev mode:**
    * `cargo watch -x run`


## Contributing

Contributions are welcome! Please open an issue or submit a pull request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.