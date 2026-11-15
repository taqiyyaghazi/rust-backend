# Tech Debt: Unauthenticated Account Enumeration on `POST /api/v1/register`

- **Severity:** Low
- **Status:** Deferred
- **Location:**
  - `crates/identity/src/presentation/http/handlers.rs` (`register_user_handler`)
  - `crates/identity/src/application/commands/register_user.rs` (`RegisterUserHandler::handle`)

## Problem

Submitting any email to `POST /api/v1/register` returns `409 Conflict` (`{"errorCode": "EMAIL_ALREADY_EXISTS"}`) when the email is already registered, and `201 Created` when it is not, with no application-level rate limiting. An unauthenticated caller can probe whether arbitrary email addresses have accounts.

## Proposed Fix

- Add IP-based rate limiting on `/api/v1/register` (at the reverse proxy/gateway or via middleware), **or**
- Return a generic `202 Accepted` response and send a verification / already-registered email out-of-band if account enumeration must be prevented completely.
