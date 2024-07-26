use std::{
    fmt::{Debug, Display},
    result,
};

use crate::{java::lang::Throwable, Java, JavaObject, Jvm, JvmOp, Local};
use crate::{AsJRef, Error};

pub(crate) trait IntoGlobal {
    type Output;

    fn into_global(self, jvm: &mut Jvm<'_>) -> Self::Output;
}

impl<J> IntoGlobal for Error<J>
where
    J: IntoGlobal<Output: AsJRef<Throwable>> + AsJRef<Throwable>,
{
    type Output = Error<J::Output>;

    fn into_global(self, jvm: &mut Jvm<'_>) -> Self::Output {
        match self {
            Error::Thrown(t) => Error::Thrown(J::into_global(t, jvm)),
            Error::SliceTooLong(s) => Error::SliceTooLong(s),
            Error::NullDeref => Error::NullDeref,
            Error::NestedUsage => Error::NestedUsage,
            Error::JvmAlreadyExists => Error::JvmAlreadyExists,
            #[cfg(feature = "dylibjvm")]
            Error::UnableToLoadLibjvm(e) => Error::UnableToLoadLibjvm(e),
            Error::JvmInternal(m) => Error::JvmInternal(m),
        }
    }
}

impl<J> IntoGlobal for Local<'_, J>
where
    J: JavaObject,
{
    type Output = Java<J>;

    fn into_global(self, jvm: &mut Jvm<'_>) -> Self::Output {
        jvm.global(&self)
    }
}

impl<J> IntoGlobal for &J
where
    J: JavaObject,
{
    type Output = Java<J>;

    fn into_global(self, jvm: &mut Jvm<'_>) -> Self::Output {
        jvm.global(&self)
    }
}

impl<J> IntoGlobal for Java<J>
where
    J: JavaObject,
{
    type Output = Java<J>;

    fn into_global(self, _jvm: &mut Jvm<'_>) -> Self::Output {
        self
    }
}

impl<J1, J2> IntoGlobal for Result<J1, J2>
where
    J1: IntoGlobal,
    J2: IntoGlobal,
{
    type Output = Result<J1::Output, J2::Output>;

    fn into_global(self, jvm: &mut Jvm<'_>) -> Self::Output {
        match self {
            Ok(o) => Ok(o.into_global(jvm)),
            Err(o) => Err(o.into_global(jvm)),
        }
    }
}

impl<J1> IntoGlobal for Option<J1>
where
    J1: IntoGlobal,
{
    type Output = Option<J1::Output>;

    fn into_global(self, jvm: &mut Jvm<'_>) -> Self::Output {
        match self {
            Some(o) => Some(o.into_global(jvm)),
            None => None,
        }
    }
}
