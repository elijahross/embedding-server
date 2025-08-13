use crate::error::{Error, Result};
use crate::model::user::Role;

#[derive(Clone, Debug)]
pub struct Ctx {
    user_id: String,

    /// Note: For the future ACS (Access Control System via API_KEY)
    conv_id: Option<Role>,
}

// Constructors.
impl Ctx {
    pub fn root_ctx() -> Self {
        Ctx {
            user_id: "roots".to_string(),
            conv_id: None,
        }
    }

    pub fn new(user_id: String, conv_id: Role) -> Result<Self> {
        if user_id == "roots" {
            Err(Error::CtxCannotNewRootCtx)
        } else {
            Ok(Self {
                user_id,
                conv_id: Some(conv_id),
            })
        }
    }

    /// Note: For the future ACS (Access Control System)
    pub fn add_conv_id(&self, conv_id: Role) -> Ctx {
        let mut ctx = self.clone();
        ctx.conv_id = Some(conv_id);
        ctx
    }
}

// Property Accessors.
impl Ctx {
    pub fn user_id(&self) -> String {
        self.user_id.clone()
    }

    //. /// Note: For the future UserRoles (Access Control System)
    pub fn conv_id(&self) -> Option<Role> {
        self.conv_id.clone()
    }
}
