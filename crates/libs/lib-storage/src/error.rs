use serde::Serialize;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Serialize, Clone)]
pub enum Error {
    // AWS Failures
    AWSClientFail(String),
    AWSClientError(String),
    AWSClientTimeout(String),
    AWSClientUnknown(String),

    ErrorCreatingBucket,
    ErrorDeletingBucket,
    ErrorListingFiles,
    ErrorDeletingFiles,
    ErrorUploadingFiles,
    ErrorDownloadingFiles,

    ErrorCreatingUploadUrl,
    ErrorSigningUrl,
    ProcessFail(String),
}

// region:    --- Error Boilerplate
impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl std::error::Error for Error {}
