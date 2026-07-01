# Feature Specification: Audit & Security Hardening

**Feature Branch**: `002-audit-security-hardening`

**Created**: 2026-07-01

**Status**: Draft

**Input**: User description: "Extend the audit trail so it tracks both login and logout events, not just login. Every user session should record when it starts and when it ends. Add a way for a logged-in user to explicitly log out, which closes their session and records the logout event. Also, harden the startup process that ensures a default administrative user always exists in the system: if the system cannot verify whether the default user already exists (due to a database problem), it must not silently assume it's safe to proceed - it should stop startup and clearly report the failure, rather than risk creating duplicate users or masking a real database issue. Every outcome of this startup check (user already exists, user was just created, or the check failed) must be clearly traceable in the system logs, without ever exposing the password. Finally, perform a security review of the authentication system (both login methods) covering at minimum: protection against repeated failed login attempts, protection against attackers learning which usernames exist based on response timing, ensuring that after logging in a user has a way to prove their identity on future requests (needed for logout to work at all), and ensuring cross-origin access to the API is explicitly and intentionally restricted rather than left wide open by default. Document findings and fix the ones that pose a real risk given this is a single-admin-user system today."

## User Scenarios & Testing

### User Story 1 - Session-based authentication with explicit logout (Priority: P1)

An authenticated user can log out of the system, which closes their active session and records the logout event in the audit trail.

**Why this priority**: Without a way to end a session, users have no way to securely disconnect. This is the foundational capability that the logout feature provides.

**Independent Test**: Can be tested by: (1) authenticating via any login method, (2) receiving a session token, (3) calling the logout endpoint with the token, (4) verifying the session is invalidated and a logout event appears in the audit log, (5) verifying the token can no longer access authenticated resources.

**Acceptance Scenarios**:

1. **Given** a user is authenticated with an active session, **When** they call the logout endpoint with their valid session token, **Then** the session is immediately invalidated and a "logout" event is recorded in the audit trail with the user identifier and timestamp.
2. **Given** a session has been logged out, **When** the same session token is used to access any authenticated resource, **Then** the request is rejected with an authentication error.
3. **Given** a user calls the logout endpoint without a valid session token, **When** the request is processed, **Then** the system returns an authentication error (no audit event is recorded for the non-existent session).

---

### User Story 2 - Audit trail records both login and logout (Priority: P1)

The audit trail captures all session lifecycle events (login and logout), giving a complete picture of when users accessed the system.

**Why this priority**: The primary purpose is to extend auditing. Making both events trackable ensures full session accountability.

**Independent Test**: Can be tested by: authenticating, logging out, and verifying the audit log contains both a login entry and a logout entry for the same user and session.

**Acceptance Scenarios**:

1. **Given** a user authenticates successfully, **When** the login completes, **Then** a "login" event is recorded in the audit trail with user identifier, authentication method, and timestamp.
2. **Given** a user logs out, **When** the logout completes, **Then** a "logout" event is recorded in the audit trail with user identifier, session reference, and timestamp.
3. **Given** a user authenticates and logs out multiple times, **When** the audit trail is queried, **Then** each login and logout appears as separate chronological entries, each with a session reference that links the matching login-logout pair.

---

### User Story 3 - Resilient startup with clear default-user seeding outcomes (Priority: P1)

When the system starts, it verifies the default administrative user exists. Every possible outcome is clearly logged, and database failures halt startup with a clear error instead of silently proceeding.

**Why this priority**: Silent failures during startup can lead to duplicate users, inconsistent state, or masked database problems. A single-admin system must fail safely.

**Independent Test**: Can be tested by: (1) starting with a fresh database and verifying the "user created" log message, (2) restarting and verifying the "user already exists" log message, (3) starting with an unreachable database and verifying the system exits with an error.

**Acceptance Scenarios**:

1. **Given** a fresh database with no users, **When** the system starts, **Then** the default admin user is created and the log contains a message indicating "default user created" (without exposing the password).
2. **Given** a database that already contains the default user, **When** the system starts, **Then** the log contains a message indicating "default user already exists" (without re-creating the user).
3. **Given** a database that is unreachable or returns errors during the user check, **When** the system starts, **Then** startup is aborted with a clear error message indicating the database check failed.

---

### User Story 4 - Rate-limited authentication with consistent response timing (Priority: P2)

The system protects against brute-force attacks by limiting repeated failed login attempts and responding with consistent timing regardless of whether a username exists.

**Why this priority**: Even with a single admin user, rate limiting and timing-safe responses prevent attackers from guessing passwords or enumerating valid usernames.

**Independent Test**: Can be tested by: (1) sending consecutive failed login attempts and verifying they are rejected after a threshold, (2) measuring response times for valid vs. invalid usernames and verifying they are indistinguishable.

**Acceptance Scenarios**:

1. **Given** more than N consecutive failed login attempts from the same IP address within M minutes, **When** the next login attempt is made, **Then** the system returns a rate-limit error.
2. **Given** a login attempt with a non-existent username, **When** the response is returned, **Then** the response time and error message are indistinguishable from a login attempt with an existing username but wrong password.
3. **Given** the rate-limit period has expired, **When** a login attempt is made, **Then** the rate-limit counter is reset and normal login processing resumes.

---

### User Story 5 - Restricted CORS policy (Priority: P2)

Cross-origin access to the API is explicitly restricted to known, trusted origins rather than left open by default.

**Why this priority**: Leaving CORS wide open exposes the authentication API to cross-origin abuse. Restricting it reduces the attack surface.

**Independent Test**: Can be tested by: (1) sending a cross-origin request from an allowed origin and verifying it succeeds, (2) sending a cross-origin request from a disallowed origin and verifying it is rejected.

**Acceptance Scenarios**:

1. **Given** a request from a configured allowed origin, **When** the request reaches the API, **Then** CORS headers permit the cross-origin access.
2. **Given** a request from an origin that is not in the allowed list, **When** the request reaches the API, **Then** CORS headers do not permit the cross-origin access.
3. **Given** no explicit CORS configuration is provided, **When** the server starts, **Then** the default policy denies all cross-origin requests (same-origin only).

### Edge Cases

- What happens when a user logs out and the session is already expired? (Should gracefully succeed with no error.)
- What happens when the audit database write for a logout event fails? (The session should still be invalidated; audit failure must not prevent logout.)
- What happens when the rate-limit counter is exceeded by a legitimate user with a shared IP (e.g., NAT)? (The counter is IP-based; a brief cooldown applies to all users from that IP.)
- What happens when the system starts and the database connection succeeds but the query times out? (The startup check has a timeout; if it triggers, startup fails with "default user check failed - database timeout".)
- What happens during concurrent startup of multiple instances against the same database? (The check + insert should be idempotent — ON CONFLICT DO NOTHING or equivalent.)

## Requirements

### Functional Requirements

- **FR-001**: The system MUST provide a logout endpoint that accepts a valid session token and invalidates the session.
- **FR-002**: The system MUST record a "logout" event in the audit trail when a session is explicitly ended via logout.
- **FR-003**: The audit trail MUST distinguish between "login", "logout", and "refresh" event types, each with: user identifier, event type, timestamp, and a session identifier that links corresponding login and logout events.
- **FR-004**: The system MUST reject requests with invalidated or expired session tokens on all authenticated endpoints.
- **FR-005**: The system MUST verify the existence of the default administrative user during startup and log the outcome (user created, user already exists, or check failed).
- **FR-006**: If the default user check fails due to a database error (connection failure, query timeout, etc.), the system MUST abort startup with a clear error message and exit.
- **FR-007**: The system MUST NOT expose the default user's password in any log message, regardless of the startup outcome.
- **FR-008**: The system MUST limit repeated failed login attempts from the same IP address to a configurable threshold within a configurable time window.
- **FR-009**: The system MUST respond to login attempts (both valid and invalid usernames, both correct and incorrect passwords) with indistinguishable response timing and identical error messages.
- **FR-010**: The system MUST provide a session token upon successful authentication that the client can present on subsequent requests to prove identity.
- **FR-011**: The system MUST restrict cross-origin requests to a configurable allow-list of origins; if no origins are configured, the default MUST be same-origin only (deny all cross-origin).
- **FR-012**: The system MUST make the allowed CORS origins configurable (e.g., via environment variable) so operators can set them without code changes.
- **FR-013**: The system MUST include session identifier and event type columns in the audit trail schema.

### Key Entities

- **User** (existing): Represents an authenticated identity. May have been created via local seeding or Google auto-registration.
- **UserSession**: Represents an active authenticated session. Created on login, invalidated on logout or expiry. Key attributes: session identifier, reference to user, creation timestamp, expiry timestamp, status (active/expired/invalidated).
- **UserAction** (extended): Audit record of an authentication event. Now tracks "login", "logout", and "refresh" event types. Key attributes: unique identifier, reference to user, reference to session, event type (login/logout/refresh), authentication method (password/google_oauth), timestamp.

## Success Criteria

### Measurable Outcomes

- **SC-001**: All authenticated sessions can be explicitly ended via the logout endpoint, and the session token is immediately invalidated upon logout.
- **SC-002**: 100% of login and logout events are recorded in the audit trail with correct event type, user, session reference, and timestamp.
- **SC-003**: The system starts successfully and logs the correct outcome (created / already exists / failure) for the default user check in under 5 seconds under normal database conditions.
- **SC-004**: The system aborts startup with a logged error within 30 seconds when the database is unreachable during the default user check.
- **SC-005**: Attackers cannot distinguish between a valid and invalid username based on login response timing or error messages (verified by automated timing analysis).
- **SC-006**: No more than N consecutive failed login attempts from a single IP are accepted within the configured time window before rate-limiting activates.
- **SC-007**: Cross-origin requests from unlisted origins are rejected; only explicitly allowed origins receive valid CORS headers.
- **SC-008**: Zero plain-text passwords appear in any log, regardless of startup outcome.

## Assumptions

- Rate-limit thresholds are configurable via environment variables with sensible defaults (e.g., 10 attempts per 5 minutes per IP).
- Session tokens use a cryptographically signed format (e.g., JWT) that can be validated without a database lookup on every request.
- Session expiry is set to a reasonable duration (e.g., 24 hours) and can be configured.
- The audit trail is append-only; records are never modified or deleted.
- The system runs as a single instance in normal operation, but the startup check is designed to be safe under concurrent instance startup (idempotent).
- CORS configuration defaults to same-origin only as a secure default; operators must explicitly configure allowed origins for cross-origin access.
- The existing login endpoints already record "login" events; this spec extends the schema to include event type and session reference.

## Out of Scope

- Password reset or change functionality
- User profile management
- Token refresh UI or client-side token management (refresh is handled server-side via `/api/auth/refresh`)
- Account lockout requiring manual admin intervention (rate-limit is self-resetting after the time window)
- Audit trail query, export, or management UI
- Multi-factor authentication
- Login via other OAuth providers
- IP-based blocking beyond rate limiting (no permanent IP bans)

## Security Review Findings

The following findings were identified during the security review of the existing authentication system:

| Finding | Severity | Description | Mitigation |
|---------|----------|-------------|------------|
| F-01: No session/token mechanism | High | Users cannot prove their identity on subsequent requests after login, making logout impossible. | Implement signed session tokens (JWT) issued at login, validated on every authenticated request. |
| F-02: No brute-force protection | Medium | Unlimited failed login attempts allow password guessing. This is partially mitigated by having a single admin user (no username enumeration possible), but the password remains guessable. | Rate-limit consecutive failed attempts per IP address. |
| F-03: No timing-safe response | Low | In a single-admin system, username enumeration is not a concern (there is only one valid username). However, response timing could theoretically reveal whether the username exists. | Apply constant-time comparison for password verification and uniform response timing. |
| F-04: CORS allows all origins | Medium | The current implementation may leave CORS headers permissive, allowing any website to make authenticated requests. | Restrict CORS to an explicit allow-list; default to same-origin only. |
| F-05: Startup silently assumes user exists on DB error | High | If the database check fails, the system currently logs the error but continues, potentially seeding a duplicate user or proceeding without critical data. | Abort startup on database errors during the user existence check. Log all outcomes clearly. |

### Severity Rationale

Given this is a **single-admin-user system**:
- **High**: Findings that could lead to unauthorized access or data corruption.
- **Medium**: Findings that increase attack surface but are partially mitigated by the single-user nature.
- **Low**: Findings that are theoretically valid but provide minimal practical advantage to an attacker.
