//! User/group operation handlers: Op::User, Op::Group

use anyhow::Result;

use crate::build::context::BuildContext;
use crate::build::users;

/// Handle Op::User: Create or update a user
pub fn handle_user(
    ctx: &BuildContext,
    name: &str,
    uid: u32,
    gid: u32,
    home: &str,
    shell: &str,
) -> Result<()> {
    users::ensure_user(&ctx.source, &ctx.staging, name, uid, gid, home, shell)?;
    Ok(())
}

/// Handle Op::Group: Create or update a group
pub fn handle_group(ctx: &BuildContext, name: &str, gid: u32) -> Result<()> {
    users::ensure_group(&ctx.source, &ctx.staging, name, gid)?;
    Ok(())
}
