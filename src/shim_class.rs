use std::sync::Arc;

use once_cell::sync::OnceCell;

use crate::find::find_constructor;
use crate::java::lang::{Class, Object};
use crate::jvm::JavaObjectExt;
use crate::raw::{IntoJniValue, MethodPtr};
use crate::{into_global::IntoGlobal as _, Java, Jvm, Local, LocalResult};
use crate::{JavaFunction, JavaObject};

pub trait ShimClassDefinition {
    const HUMAN_NAME: &'static str;

    fn class_cell() -> &'static OnceCell<Java<Class>>;
    fn constructor_cell() -> &'static OnceCell<MethodPtr>;
    fn class<'jvm>(jvm: &mut Jvm<'jvm>) -> LocalResult<'jvm, Local<'jvm, Class>>;
    fn java_functions() -> Vec<JavaFunction>;

    /// Creates a shim class wrapping `object` with the type `J`.
    ///
    /// # Safety
    ///
    /// The Java type `J` must be appropriate.
    /// It is typically an interface implemented by the shim class.
    unsafe fn instantiate_shim_for<'jvm, R, J>(
        jvm: &mut Jvm<'jvm>,
        object: R,
    ) -> crate::LocalResult<'jvm, Local<'jvm, J>>
    where
        J: JavaObject,
    {
        // Load the (cached) class/constructor pointers
        let class = Self::cached_class(jvm)?;
        let constructor = Self::constructor_cell().get_or_try_init(|| {
            find_constructor(jvm, &class, unsafe {
                ::core::ffi::CStr::from_bytes_with_nul_unchecked(b"(J)V\0")
            })
        })?;
        let env = jvm.env();

        // Allocate a `RawArc` storing `object` and attempt to create an
        // instance of the shim class taking ownership of the pointer (an i64).
        let arc_object = RawArc::new(object);
        let shim_obj: Option<Local<'jvm, Object>> = unsafe {
            env.invoke(
                |env| env.NewObjectA,
                |env, f| {
                    f(
                        env,
                        JavaObjectExt::as_raw(&**class).as_ptr(),
                        constructor.as_ptr(),
                        [IntoJniValue::into_jni_value(arc_object.as_i64())].as_ptr(),
                    )
                },
            )
        }?;

        // NewObjectA should only return a null pointer when an exception occurred in the
        // constructor, so reaching here is a strange JVM state.
        let Some(shim_obj) = shim_obj else {
            return Err(crate::Error::JvmInternal(format!(
                "constructor failed creating class `{}`",
                Self::HUMAN_NAME,
            )));
        };

        // Now that the argument has been successfully constructed, we can forget the RawArc.
        // The ref count is transferred to `shim_obj`.
        std::mem::forget(arc_object);

        // Cast this to the java interface type it is supposed to be.
        //
        // SAFETY: Function contract.
        unsafe {
            Ok(std::mem::transmute::<Local<'jvm, Object>, Local<'jvm, J>>(
                shim_obj,
            ))
        }
    }

    /// Returns the cached class for this shim class.
    /// The class is created lazily and cached.
    fn cached_class<'jvm>(jvm: &mut Jvm<'jvm>) -> crate::LocalResult<'jvm, &'static Java<Class>> {
        Self::class_cell().get_or_try_init(|| {
            let class = Self::class(jvm)?;
            jvm.register_native_methods(&Self::java_functions())?;
            Ok(class.into_global(jvm))
        })
    }
}

struct RawArc<R> {
    ptr: *const R,
}

impl<R> RawArc<R> {
    fn new(data: R) -> Self {
        Self {
            ptr: Arc::into_raw(Arc::new(data)),
        }
    }

    fn as_i64(&self) -> i64 {
        self.ptr as usize as i64
    }
}

impl Drop for RawArc<R> {
    fn drop(&mut self) {
        unsafe {
            Arc::from_raw(self.ptr);
        }
    }
}
