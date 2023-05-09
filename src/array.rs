use std::marker::PhantomData;

use crate::{
    cast::Upcast,
    error::check_exception,
    java::{self, lang::Class},
    plumbing::JavaObjectExt,
    raw::{HasEnvPtr, ObjectPtr},
    Error, IntoRust, JavaObject, JavaType, Jvm, JvmOp, Local, ScalarMethod,
};

pub struct JavaArray<T: JavaType> {
    _element: PhantomData<T>,
}

unsafe impl<T: JavaType> JavaObject for JavaArray<T> {
    fn class<'jvm>(jvm: &mut Jvm<'jvm>) -> crate::Result<'jvm, Local<'jvm, Class>> {
        T::array_class(jvm)
    }
}

// Upcasts
unsafe impl<T: JavaType> Upcast<JavaArray<T>> for JavaArray<T> {}
// all arrays extend Object
unsafe impl<T: JavaType> Upcast<java::lang::Object> for JavaArray<T> {}

// array.length isn't a normal field or method, so hand-generating the traits
pub trait JavaArrayExt<T: JavaType>: JvmOp {
    type Length: ScalarMethod<Self, jni_sys::jsize>;
    fn length(self) -> Self::Length;
}

impl<This, T> JavaArrayExt<T> for This
where
    This: JvmOp,
    for<'jvm> This::Output<'jvm>: AsRef<JavaArray<T>>,
    T: JavaType,
{
    type Length = Length<Self, T>;
    fn length(self) -> Self::Length {
        Length {
            this: self,
            element: PhantomData,
        }
    }
}

#[derive(Clone)]
pub struct Length<This, T> {
    this: This,
    element: PhantomData<T>,
}

impl<This, T> JvmOp for Length<This, T>
where
    This: JvmOp,
    for<'jvm> This::Output<'jvm>: AsRef<JavaArray<T>>,
    T: JavaType,
{
    type Output<'jvm> = jni_sys::jsize;

    fn execute<'jvm>(self, jvm: &mut Jvm<'jvm>) -> crate::Result<'jvm, Self::Output<'jvm>> {
        let this = self.this.execute(jvm)?;
        let this = this.as_ref().as_raw();

        let len = unsafe {
            jvm.env()
                .invoke(|env| env.GetArrayLength, |env, f| f(env, this.as_ptr()))
        };
        Ok(len)
    }
}

macro_rules! primivite_array {
    ($([$rust:ty]: $java_name:literal $java_ty:ident $new_fn:ident $get_fn:ident $set_fn:ident,)*) => {
        $(
            impl JvmOp for &[$rust] {
                type Output<'jvm> = Local<'jvm, JavaArray<$rust>>;

                fn execute<'jvm>(self, jvm: &mut Jvm<'jvm>) -> crate::Result<'jvm, Self::Output<'jvm>> {
                    let Ok(len) = self.len().try_into() else {
                        return Err(Error::SliceTooLong(self.len()))
                    };

                    let env = jvm.env();
                    let array = unsafe { env.invoke(|env| env.$new_fn, |env, f| f(env, len)) };
                    if let Some(array) = ObjectPtr::new(array) {
                        unsafe {
                            env.invoke(|env| env.$set_fn, |env, f| f(
                                env,
                                array.as_ptr(),
                                0,
                                len,
                                self.as_ptr().cast::<jni_sys::$java_ty>(),
                            ));
                        }

                        unsafe { Ok(Local::from_raw(env, array)) }
                    } else {
                        check_exception(jvm)?; // Likely threw OutOfMemoryError
                        return Err(Error::JvmInternal(format!(
                            "failed to allocate `{}[{}]`",
                            $java_name,
                            len
                        )));
                    }
                }
            }

            impl<J> IntoRust<Vec<$rust>> for J
            where
                for<'jvm> J: JvmOp,
                for<'jvm> J::Output<'jvm>: AsRef<JavaArray<$rust>>,
            {
                fn into_rust<'jvm>(self, jvm: &mut Jvm<'jvm>) -> $crate::Result<'jvm, Vec<$rust>> {
                    let array = self.execute(jvm)?;
                    let array = jvm.local(array.as_ref());

                    let len = array.length().execute(jvm)?;
                    let mut vec = Vec::<$rust>::with_capacity(len as usize);

                    unsafe {
                        jvm.env().invoke(|env| env.$get_fn, |env, f| f(
                            env,
                            array.as_raw().as_ptr(),
                            0,
                            len,
                            vec.as_mut_ptr().cast::<jni_sys::$java_ty>(),
                        ));
                        vec.set_len(len as usize);
                    }
                    check_exception(jvm)?;

                    Ok(vec)
                }
            }
        )*
    };
}

// Bool is represented as u8 in JNI
primivite_array! {
    [i8]: "boolean" jboolean NewBooleanArray GetBooleanArrayRegion SetBooleanArrayRegion,
    [u16]: "char" jchar NewCharArray GetCharArrayRegion SetCharArrayRegion,
    [i16]: "short" jshort NewShortArray GetShortArrayRegion SetShortArrayRegion,
    [i32]: "int" jint NewIntArray GetIntArrayRegion SetIntArrayRegion,
    [i64]: "long" jlong NewLongArray GetLongArrayRegion SetLongArrayRegion,
    [f32]: "float" jfloat NewFloatArray GetFloatArrayRegion SetFloatArrayRegion,
    [f64]: "double" jdouble NewDoubleArray GetDoubleArrayRegion SetDoubleArrayRegion,
}
