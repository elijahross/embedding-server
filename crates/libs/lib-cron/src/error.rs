pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub enum Error {
    MissingEnv(&'static str),
    ChronFails(String),
    Custom(String),
}

// region:    --- Error Boilerplate
impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl std::error::Error for Error {}

impl From<lib_utils::error::Error> for Error {
    fn from(err: lib_utils::error::Error) -> Self {
        match err {
            _ => Error::Custom(err.to_string()),
        }
    }
}
