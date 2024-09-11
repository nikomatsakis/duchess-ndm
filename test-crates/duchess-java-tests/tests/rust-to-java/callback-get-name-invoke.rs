//@run

use duchess::java;
use duchess::prelude::*;
use std::sync::Arc;

struct Callback {
    last_name: String,
}

impl Drop for Callback {
    fn drop(&mut self) {
        // panic!("callback drop");
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

include!(concat!(env!("OUT_DIR"), "/Shim__callback__GetName.rs"));

impl duchess::IntoJava<callback::GetName> for Callback {
    type JvmOp = ToJavaInterface;

    fn into_op(self) -> Self::JvmOp {
        eprintln!("A");
        ToJavaInterface { cb: Arc::new(self) }
    }
}

impl duchess::JvmOp for ToJavaInterface {
    type Output<'jvm> = duchess::Local<'jvm, callback::GetName>;

    fn do_jni<'jvm>(
        self,
        jvm: &mut duchess::Jvm<'jvm>,
    ) -> duchess::LocalResult<'jvm, Self::Output<'jvm>> {
        eprintln!("X");
        let value = self.cb.clone();
        eprintln!("Y {}", Arc::strong_count(&value));
        let result = unsafe {
            SHIM_CALLBACK_GET_NAME.instantiate_shim_for::<_, callback::GetName>(jvm, value)
        };
        eprintln!("Z {}", Arc::strong_count(&self.cb));
        result
    }
}

#[duchess::java_function(duchess_util.Shim__callback__GetName::native__getName)]
fn get_name_native(name: &java::lang::String, native_pointer: i64) -> duchess::Result<String> {
    let native_pointer: *mut Callback = native_pointer as usize as *mut Callback;
    let callback = unsafe { &*native_pointer };
    let name: String = name.execute()?;
    Ok(format!("{name} {}", callback.last_name))
}

// #[duchess::java_function(duchess_util.Shim__callback__GetName::native__drop)]
// fn drop_native(native_pointer: i64) -> () {
//     let native_pointer: *mut Callback = native_pointer as usize as *mut Callback;
//     unsafe {
//         Arc::from_raw(native_pointer);
//     }
// }

fn main() -> duchess::Result<()> {
    duchess::Jvm::builder()
        .link(vec![get_name_native::java_fn()])
        .try_launch()?;

    eprintln!("{:?}", std::env::var("CLASSPATH"));

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
