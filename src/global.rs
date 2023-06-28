use std::{collections::HashMap, hash::BuildHasher};

use crate::{Global, JavaObject, Jvm, JvmOp, Local};

/// [`JvmOp`][] that converts a local result into a global one.
#[derive_where::derive_where(Copy, Clone)]
pub struct GlobalOp<J: JvmOp> {
    j: J,
}

impl<J: JvmOp> GlobalOp<J>
where
    J: JvmOp,
    for<'jvm> <J as JvmOp>::Output<'jvm>: IntoGlobal<'jvm>,
{
    pub(crate) fn new(j: J) -> Self {
        Self { j }
    }
}

impl<J> JvmOp for GlobalOp<J>
where
    J: JvmOp,
    for<'jvm> J::Output<'jvm>: IntoGlobal<'jvm>,
{
    type Output<'jvm> = GlobalVersionOf<'jvm, J::Output<'jvm>>;

    fn execute_with<'jvm>(
        self,
        jvm: &mut crate::Jvm<'jvm>,
    ) -> crate::Result<'jvm, Self::Output<'jvm>> {
        let local = self.j.execute_with(jvm)?;
        local.into_global(jvm)
    }
}

pub type GlobalVersionOf<'jvm, T> = <T as IntoGlobal<'jvm>>::Output;

/// Converts a value of type `Self`, which may be local to `jvm`, into a global JVM reference,
/// which must be independent from `'jvm`.
///
/// If `Self::Output = Self`, then this must be a no-op conversion.
pub trait IntoGlobal<'jvm> {
    type Output;

    fn into_global(self, jvm: &mut Jvm<'jvm>) -> crate::Result<'jvm, Self::Output>;
}

impl<'jvm, T> IntoGlobal<'jvm> for Global<T>
where
    T: JavaObject,
{
    type Output = Global<T>;

    fn into_global(self, _jvm: &mut Jvm<'jvm>) -> crate::Result<'jvm, Self::Output> {
        Ok(self)
    }
}

impl<'jvm, T> IntoGlobal<'jvm> for Local<'jvm, T>
where
    T: JavaObject,
{
    type Output = Global<T>;

    fn into_global(self, jvm: &mut Jvm<'jvm>) -> crate::Result<'jvm, Self::Output> {
        Ok(jvm.global::<T>(&self))
    }
}

impl<'jvm, T> IntoGlobal<'jvm> for &T
where
    T: JavaObject,
{
    type Output = Global<T>;

    fn into_global(self, jvm: &mut Jvm<'jvm>) -> crate::Result<'jvm, Self::Output> {
        Ok(jvm.global::<T>(self))
    }
}

macro_rules! identity_globals {
    ($($rust:ty,)*) => {
        $(
            impl<'jvm> IntoGlobal<'jvm> for $rust {
                type Output = $rust;

                fn into_global(self, _jvm: &mut Jvm<'jvm>) -> crate::Result<'jvm, Self::Output> {
                    Ok(self)
                }
            }
        )*
    };
}

identity_globals! {
    String,
    (),
    bool,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    i64,
    u64,
    f32,
    f64,
    i128,
    u128,
}

impl<'jvm, T> IntoGlobal<'jvm> for Option<T>
where
    T: IntoGlobal<'jvm>,
{
    type Output = Option<T::Output>;

    fn into_global(self, jvm: &mut Jvm<'jvm>) -> crate::Result<'jvm, Self::Output> {
        // FIXME: Lacking specialization here is *mildly* painful
        match self {
            None => Ok(None),
            Some(p) => Ok(Some(p.into_global(jvm)?)),
        }
    }
}

impl<'jvm, O, E> IntoGlobal<'jvm> for Result<O, E>
where
    O: IntoGlobal<'jvm>,
    E: IntoGlobal<'jvm>,
{
    type Output = Result<O::Output, E::Output>;

    fn into_global(self, jvm: &mut Jvm<'jvm>) -> crate::Result<'jvm, Self::Output> {
        // FIXME: Lacking specialization here is *mildly* painful
        match self {
            Ok(o) => Ok(Ok(o.into_global(jvm)?)),
            Err(e) => Ok(Err(e.into_global(jvm)?)),
        }
    }
}

impl<'jvm, T> IntoGlobal<'jvm> for Vec<T>
where
    T: IntoGlobal<'jvm>,
{
    type Output = Vec<T::Output>;

    fn into_global(self, jvm: &mut Jvm<'jvm>) -> crate::Result<'jvm, Self::Output> {
        // FIXME: Ugh, lacking specialization here is *really* painful.
        self.into_iter().map(|e| e.into_global(jvm)).collect()
    }
}

impl<'jvm, K, V, S> IntoGlobal<'jvm> for HashMap<K, V, S>
where
    K: IntoGlobal<'jvm>,
    V: IntoGlobal<'jvm>,
    K::Output: std::hash::Hash + Eq,
    V::Output: std::hash::Hash + Eq,
    S: Default + BuildHasher,
{
    type Output = HashMap<K::Output, V::Output, S>;

    fn into_global(self, jvm: &mut Jvm<'jvm>) -> crate::Result<'jvm, Self::Output> {
        let mut output = HashMap::with_capacity_and_hasher(self.len(), S::default());
        for (key, value) in self {
            let key = key.into_global(jvm)?;
            let value = value.into_global(jvm)?;
            output.insert(key, value);
        }
        Ok(output)
    }
}
