use crate::application::ports::admin_repository::{AdminRepository, ListUsersResult};
use crate::domain::errors::AdminError;

/// Default page size when the client omits `per_page` (see #101).
pub const DEFAULT_PAGE_SIZE: u32 = 100;
/// Hard cap on `per_page` so a single request can never materialize more than
/// this many rows in memory.
pub const MAX_PAGE_SIZE: u32 = 500;

/// Returns the requested user page and its total count.
///
/// Callers must validate that `page` is 1-based and normalize `per_page` to the
/// supported range before invoking this use case.
pub async fn list_users(
    repo: &dyn AdminRepository,
    page: u32,
    per_page: u32,
) -> Result<ListUsersResult, AdminError> {
    repo.list_users(page, per_page).await
}
