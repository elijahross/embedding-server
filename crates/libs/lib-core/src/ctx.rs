use crate::error::{Error, Result};
use crate::model::user::Role;

#[derive(Clone, Debug)]
pub struct Ctx {
    user_id: String,
    /// Note: For the future ACS (Access Control System via API_KEY or Token)
    role: Option<Role>,
}

// Constructors.
impl Ctx {
    pub fn root_ctx() -> Self {
        Ctx {
            user_id: "roots".to_string(),
            role: None,
        }
    }

    pub fn new(user_id: String, role: Option<Role>) -> Result<Self> {
        if user_id == "roots" {
            Err(Error::CtxCannotNewRootCtx)
        } else {
            Ok(Self { user_id, role })
        }
    }

    /// Note: For the future ACS (Access Control System)
    pub fn add_role(&self, role: Role) -> Ctx {
        let mut ctx = self.clone();
        ctx.role = Some(role);
        ctx
    }
}

// Property Accessors.
impl Ctx {
    pub fn user_id(&self) -> String {
        self.user_id.clone()
    }

    /// Note: For the future UserRoles (Access Control System)
    pub fn role(&self) -> Option<Role> {
        self.role.clone()
    }
}
