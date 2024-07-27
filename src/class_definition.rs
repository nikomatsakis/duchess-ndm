use std::sync::Arc;

use once_cell::sync::OnceCell;

use crate::{
    find::find_constructor,
    into_global::IntoGlobal as _,
    java::lang::{Class, Object},
    jvm::JavaObjectExt,
    raw::{IntoJniValue, MethodPtr},
    Java, JavaObject, Jvm, Local,
};

pub struct ClassDefinition {
    human_class_name: &'static str,
    sync: OnceCell<Java<Class>>,
    constructor: OnceCell<MethodPtr>,
    data: ClassDefinitionData,
}

enum ClassDefinitionData {
    Const {
        jni_class_name: &'static str,
        bytecode: &'static [i8],
    },
}

impl ClassDefinition {
    /// Create a new "shim class" definition, intended to be stored in a `STATIC`.
    /// Calls to this constructor are created via duchess's build-rs plumbing and macros.
    ///
    /// Shim classes are autogeneated classes that hold a reference to a Rust object
    /// and implement some Java interface; every Java interface method delegates to a native
    /// method that calls into Rust.
    ///
    /// # Parameters
    ///
    /// * `human_class_name`, human readable class name for the shim (e.g., `duchess_util.Foo`)
    /// * `jni_class_name`, the shim class name prepared for JNI (e.g., `duchess_util/Foo`)
    /// * `bytecode`, bytecode of the class
    pub const fn new(
        human_class_name: &'static str,
        jni_class_name: &'static str,
        bytecode: &'static [i8],
    ) -> Self {
        Self {
            human_class_name,
            constructor: OnceCell::new(),
            sync: OnceCell::new(),
            data: ClassDefinitionData::Const {
                jni_class_name,
                bytecode,
            },
        }
    }

    pub fn jni_class_name(&self) -> &str {
        match &self.data {
            ClassDefinitionData::Const {
                jni_class_name,
                bytecode: _,
            } => &jni_class_name,
        }
    }

    pub fn bytecode(&self) -> &[i8] {
        match &self.data {
            ClassDefinitionData::Const {
                jni_class_name: _,
                bytecode,
            } => bytecode,
        }
    }

    pub fn register(&self) -> crate::Result<&Java<Class>> {
        Jvm::with(|jvm| self.register_with(jvm))
    }

    fn register_with<'jvm>(&self, jvm: &mut Jvm<'jvm>) -> crate::LocalResult<'jvm, &Java<Class>> {
        self.sync.get_or_try_init(|| {
            jvm.define_class(self.jni_class_name(), self.bytecode())
                .map(|j| j.into_global(jvm))
        })
    }

    /// Creates a shim class wrapping `object` with the type `J`.
    ///
    /// # Safety
    ///
    /// The Java type `J` must be appropriate.
    /// It is typically an interface implemented by the shim class.
    pub unsafe fn instantiate_shim_for<'jvm, R, J>(
        &self,
        jvm: &mut Jvm<'jvm>,
        object: Arc<R>,
    ) -> crate::LocalResult<'jvm, Local<'jvm, J>>
    where
        J: JavaObject,
    {
        let class = self.register_with(jvm)?;
        let constructor = self.constructor.get_or_try_init(|| {
            find_constructor(jvm, &class, unsafe {
                ::core::ffi::CStr::from_bytes_with_nul_unchecked(b"(J)V\0")
            })
        })?;
        let env = jvm.env();
        let object = Arc::into_raw(object) as usize as i64;
        let obj: Option<Local<'jvm, Object>> = unsafe {
            env.invoke(
                |env| env.NewObjectA,
                |env, f| {
                    f(
                        env,
                        JavaObjectExt::as_raw(&**class).as_ptr(),
                        constructor.as_ptr(),
                        [IntoJniValue::into_jni_value(object)].as_ptr(),
                    )
                },
            )
        }?;
        obj.map(|obj| unsafe { std::mem::transmute::<Local<'jvm, Object>, Local<'jvm, J>>(obj) })
            .ok_or_else(|| {
                // NewObjectA should only return a null pointer when an exception occurred in the
                // constructor, so reaching here is a strange JVM state
                crate::Error::JvmInternal(format!(
                    "constructor failed creating class `{}`",
                    self.human_class_name,
                ))
            })
    }
}
