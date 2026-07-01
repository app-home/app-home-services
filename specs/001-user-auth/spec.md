# User Authentication

## Overview

This feature enables user authentication through two distinct methods: traditional username/password login and Google OAuth (Sign in with Google). A single pre-seeded default user exists for the username/password method with no self-registration flow. Users logging in via Google for the first time are automatically registered. Every successful login, regardless of method, is recorded in a user_actions audit table for traceability.

## Actors

- **Anonymous User**: A visitor who has not yet authenticated. May log in via username/password or Google OAuth.
- **Default User**: A single pre-configured user account with username/password credentials, seeded into the system.
- **Google-authenticated User**: A user who authenticates via their Google account. If this is their first login, they are automatically created in the system.

## User Scenarios

### Scenario 1: Default user logs in via username/password
1. The user navigates to the login screen.
2. The user enters the pre-seeded username and password.
3. The system validates the credentials against the stored hashed password.
4. Upon successful validation, the system creates a session and records the login in user_actions with auth method "password".
5. The user is granted access to the application.

### Scenario 2: Returning Google user logs in via Google OAuth
1. The user navigates to the login screen.
2. The user selects "Sign in with Google".
3. The user is redirected to Google's authentication page and grants permission.
4. Google returns an identity token to the system.
5. The system verifies the token and finds an existing user record matching the Google account email.
6. The user is authenticated and the login is recorded in user_actions with auth method "google_oauth".
7. The user is granted access to the application.

### Scenario 3: First-time Google user logs in (auto-registration)
1. The user navigates to the login screen.
2. The user selects "Sign in with Google".
3. The user authenticates with Google and grants permission.
4. Google returns an identity token to the system.
5. The system verifies the token but finds no existing user record matching the Google account email.
6. The system automatically creates a new user record using profile information from Google.
7. The user is authenticated and the login is recorded in user_actions with auth method "google_oauth".
8. The user is granted access to the application.

### Scenario 4: Failed login attempt (incorrect password)
1. The user navigates to the login screen.
2. The user enters a username that exists but an incorrect password.
3. The system rejects the login attempt and displays an appropriate error message.
4. No entry is recorded in user_actions (only successful logins are recorded).

### Scenario 5: Failed login attempt (nonexistent username/password)
1. The user navigates to the login screen.
2. The user enters a username that does not exist with any password.
3. The system rejects the login attempt and displays a generic error message (not revealing whether the username exists).
4. No entry is recorded in user_actions.

### Scenario 6: Failed Google login (invalid or expired token)
1. The user selects "Sign in with Google".
2. Google returns an invalid or expired token.
3. The system detects the invalid token and rejects the authentication attempt.
4. A generic error message is shown to the user.
5. No entry is recorded in user_actions.

## Functional Requirements

### R1: Username/Password Login
R1.1: The system shall accept a username and password combination and validate it against stored credentials.
R1.2: Only the pre-seeded default user shall be able to log in via username/password — no self-registration is supported for this method.
R1.3: The system shall not reveal whether a given username exists in the system; error messages must be identical for "user not found" and "incorrect password" scenarios.
R1.4: Passwords shall be stored in hashed form only. Plain-text passwords must never be persisted, logged, or returned in any response.
R1.5: The system shall enforce that the password field is never returned in any API response or log output.

### R2: Google OAuth Login
R2.1: The system shall provide a "Sign in with Google" option on the login screen.
R2.2: The system shall initiate the Google OAuth 2.0 flow when the user selects this option.
R2.3: The system shall verify the identity token returned by Google.
R2.4: If a user record already exists matching the Google account email, the system shall authenticate that existing user.
R2.5: If no user record exists matching the Google account email, the system shall automatically create a new user record using the profile information from Google (name, email) and then authenticate the new user.

### R3: Audit Logging
R3.1: Every successful login shall be recorded in a user_actions audit table.
R3.2: Each audit record shall contain at minimum: the authenticated user identifier, the authentication method used ("password" or "google_oauth"), and the timestamp of the login.
R3.3: Failed login attempts must not be recorded in user_actions.

### R4: Default User Seeding
R4.1: A single default user shall be pre-seeded into the system upon initial setup.
R4.2: The default user's password shall be stored as a hash, not in plain text.
R4.3: The default user credentials shall be configurable (not hard-coded) to allow changing them without modifying the application code.

## Key Entities

### User
- Unique identifier
- Username (for the default user; may be null for Google-only users)
- Email address
- Display name
- Password hash (null for users who have only logged in via Google)
- Auth provider identifier (to distinguish local vs. Google accounts)
- Creation timestamp
- Last updated timestamp

### UserAction (audit log)
- Unique identifier
- Reference to the authenticated user
- Authentication method used ("password" or "google_oauth")
- Timestamp of the action

## Non-functional Requirements

### Security
- All passwords must be hashed using a strong, industry-standard hashing algorithm (e.g., bcrypt, Argon2).
- Google OAuth token verification must validate the token's signature, issuer, and audience.
- Authentication endpoints must be protected against brute-force attacks (reasonable rate limiting).

### Performance
- The authentication flow (login + audit write) should complete within 2 seconds under normal conditions.

## Assumptions

- The default user's credentials are provided via configuration (environment variables or equivalent) and seeded during initial database setup.
- The Google OAuth flow uses standard OAuth 2.0 with OpenID Connect for identity verification.
- The system has internet access to verify Google's OAuth tokens using Google's public keys.
- Session management (tokens, cookies) is handled separately and is out of scope for this specification.
- The user_actions table is append-only; records are never modified or deleted.

## Out of Scope

- Self-registration or user sign-up via username/password
- Password reset or change functionality
- Session management (token creation, refresh, revocation)
- Role-based access control or user permissions
- Account deactivation or deletion
- Multi-factor authentication
- Login via other OAuth providers (Facebook, GitHub, etc.)

## Success Criteria

1. **Login success rate**: 100% of valid credential submissions result in successful authentication.
2. **Audit completeness**: 100% of successful logins are recorded in user_actions with correct user, method, and timestamp.
3. **Google auto-registration**: 100% of first-time Google OAuth logins create a user record and successfully authenticate.
4. **Password security**: Zero plain-text passwords exist in any database, log, or API response.
5. **Error handling**: All failed login attempts return a generic error message without revealing whether the username exists.
6. **Response time**: Authentication completes within 2 seconds for 95% of requests under normal load.
