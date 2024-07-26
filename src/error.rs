use std::{
    fmt::{Debug, Display},
    result,
};

use thiserror::Error;

use crate::AsJRef;
use crate::{java::lang::Throwable, Java, Jvm, JvmOp, Local};

/// Result returned by most Java operations that may contain a local reference
/// to a thrown exception.
pub type LocalResult<'jvm, T> = result::Result<T, Error<Local<'jvm, Throwable>>>;

/// Result returned by [`crate::Jvm::with()`] that will store any uncaught
/// exception as a global reference.
pub type Result<T> = result::Result<T, Error<Java<Throwable>>>;

#[derive(Error)]
pub enum Error<T: AsJRef<Throwable>> {
    /// A reference to an uncaught Java exception
    #[error("Java invocation threw: {}", try_extract_message(.0))]
    Thrown(T),

    #[error(
        "slice was too long (`{0}`) to convert to a Java array, which are limited to `i32::MAX`"
    )]
    SliceTooLong(usize),

    #[error("attempted to deref a null Java object pointer")]
    NullDeref,

    #[error("attempted to nest `Jvm::with` calls")]
    NestedUsage,

    #[error("JVM already exists")]
    JvmAlreadyExists,

    #[cfg(feature = "dylibjvm")]
    #[error(transparent)]
    UnableToLoadLibjvm(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("{0}")]
    JvmInternal(String),
}

fn try_extract_message(exception: &impl AsJRef<Throwable>) -> String {
    let result =
        || -> crate::Result<_> { exception.as_jref()?.to_string().assert_not_null().execute() };
    result().unwrap_or_else(|err| format!("failed to get message: {}", err))
}

impl<T> Debug for Error<T>
where
    T: AsJRef<Throwable>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}
