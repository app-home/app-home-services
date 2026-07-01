# app-home-services

User authentication service supporting local password login and Google OAuth.

## Requirements

- Rust 2024 edition (nightly)
- PostgreSQL 14+

## Setup

1. **Configure environment**

   ```bash
   cp .env.example .env
   # Edit .env with your database URL and secrets
   ```

2. **Create the database**

   ```bash
   createdb app_home
   ```

3. **Run**

   Make sure PostgreSQL is running, then start the service:

   ```bash
   cargo run
   ```

   Migrations are applied automatically on startup (via `sqlx::migrate!`). On first run, the default local user is also seeded.

## API Endpoints

| Method | Path                        | Description                    |
|--------|-----------------------------|--------------------------------|
| POST   | `/api/auth/login/password`  | Login with username/password   |
| POST   | `/api/auth/login/google`    | Login with Google OAuth token  |
| GET    | `/api/health`               | Health check                   |
