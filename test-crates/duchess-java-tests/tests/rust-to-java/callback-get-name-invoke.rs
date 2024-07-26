//@run

use duchess::java;
use duchess::prelude::*;
use std::sync::Arc;

struct Callback {
    last_name: String,
}

impl Drop for Callback {
    fn drop(&mut self) {
        eprintln!("callback drop");
    }
}

#[duchess::impl_java_interface]
impl callback::GetName for Callback {
    fn get_name(&self, name: &java::lang::String) -> duchess::GlobalResult<String> {
        let name: String = name.to_rust().execute()?;
        Ok(format!("{name} {}", self.last_name))
    }
}

#[derive(Clone)]
pub struct ToJavaInterface {
    cb: Arc<Callback>,
}

duchess::java_package! {
    package callback;

    public interface callback.GetName {
        public abstract java.lang.String getName(java.lang.String);
    }
    public class callback.GetNameFrom {
        public callback.GetNameFrom();
        public java.lang.String getNameFrom(callback.GetName);
    }
}

duchess::java_package! {
    package duchess_util;

    public class duchess_util.Shim__callback__GetName implements callback.GetName {
        long nativePointer;
        static java.lang.ref.Cleaner cleaner;
        public duchess.Shim__callback__GetName(long);
        static native void native__drop(long);
        static native java.lang.String native__getName(java.lang.String, long);
        public java.lang.String getName(java.lang.String);
        static {};
    }
}

impl duchess::IntoJava<callback::GetName> for Callback {
    type JvmOp = ToJavaInterface;

    fn into_op(self) -> Self::JvmOp {
        ToJavaInterface { cb: Arc::new(self) }
    }
}

impl duchess::JvmOp for ToJavaInterface {
    type Output<'jvm> = duchess::Local<'jvm, callback::GetName>;

    fn do_jni<'jvm>(
        self,
        jvm: &mut duchess::Jvm<'jvm>,
    ) -> duchess::LocalResult<'jvm, Self::Output<'jvm>> {
        let value = self.cb.clone();
        let value_long: i64 = Arc::into_raw(value) as usize as i64;
        duchess_util::Shim__callback__GetName::new(value_long)
            .upcast()
            .do_jni(jvm)
    }
}

#[duchess::java_function(duchess_util.Shim__callback__GetName::native__getName)]
fn get_name_native(name: &java::lang::String, native_pointer: i64) -> duchess::Result<String> {
    let native_pointer: *mut Callback = native_pointer as usize as *mut Callback;
    let callback = unsafe { &*native_pointer };
    let name: String = name.execute()?;
    Ok(format!("{name} {}", callback.last_name))
}

#[duchess::java_function(duchess_util.Shim__callback__GetName::native__drop)]
fn drop_native(native_pointer: i64) -> () {
    let native_pointer: *mut Callback = native_pointer as usize as *mut Callback;
    unsafe {
        Arc::from_raw(native_pointer);
    }
}

fn main() -> duchess::Result<()> {
    duchess::Jvm::builder()
        .link(vec![get_name_native::java_fn()])
        .try_launch()?;

    let get_name_from = callback::GetNameFrom::new().execute()?;

    // wrap the Rust box in an instance of `Dummy`
    let result: String = get_name_from
        .get_name_from(Callback {
            last_name: "Bueller".to_string(),
        })
        .assert_not_null()
        .execute()?;

    assert_eq!(result, "Ferris Bueller");

    Ok(())
}
