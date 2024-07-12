//@run

use duchess::java;
use duchess::prelude::*;
use std::sync::Arc;

duchess::java_package! {
    package callback;

    class Callback { * }
    class CallCallback { * }
    class Dummy { * }
}

struct Callback {
    last_name: String,
}

impl Drop for Callback {
    fn drop(&mut self) {
        eprintln!("callback drop");
    }
}

#[duchess::impl_java_interface(callback.Callback)]
impl JavaInterface for ToJavaInterface {
    fn get_name(
        &self,
        name: &java::lang::String,
    ) -> duchess::GlobalResult<String> {
        let name: String = name.to_rust().execute()?;
        Ok(format!("{name} {}", self.last_name))
    }
}

#[derive(Clone)]
pub struct ToJavaInterface {
    cb: Arc<Callback>,
}

impl duchess::IntoJava<callback::Callback> for Callback {
    type JvmOp = ToJavaInterface;

    fn into_op(self) -> Self::JvmOp {
        ToJavaInterface { cb: Arc::new(self) }
    }
}

impl duchess::JvmOp for ToJavaInterface {
    type Output<'jvm> = duchess::Local<'jvm, callback::Dummy>;

    fn execute_with<'jvm>(
        self,
        jvm: &mut duchess::Jvm<'jvm>,
    ) -> duchess::Result<'jvm, Self::Output<'jvm>> {
        let value = self.cb.clone();
        let value_long: i64 = Arc::into_raw(value) as usize as i64;
        callback::Dummy::new(value_long).execute_with(jvm)
    }
}

#[duchess::java_function(callback.Dummy::getNameNative)]
fn get_name_native(
    _this: &callback::Dummy,
    name: &java::lang::String,
    native_pointer: i64,
) -> duchess::GlobalResult<String> {
    let native_pointer: *mut Callback = native_pointer as usize as *mut Callback;
    let callback = unsafe { &*native_pointer };
    let name: String = name.to_rust().execute()?;
    Ok(format!("{name} {}", callback.last_name))
}

#[duchess::java_function(callback.Dummy::drop)]
fn drop_native(native_pointer: i64) -> () {
    let native_pointer: *mut Callback = native_pointer as usize as *mut Callback;
    unsafe {
        Arc::from_raw(native_pointer);
    }
}

fn main() -> duchess::GlobalResult<()> {
    duchess::Jvm::builder()
        .link(vec![get_name_native::java_fn()])
        .try_launch()?;

    let ccb = callback::CallCallback::new().global().execute()?;

    // wrap the Rust box in an instance of `Dummy`
    let result: String = ccb
        .method(Callback {
            last_name: "Rustacean".to_string(),
        })
        .assert_not_null()
        .to_rust()
        .execute()?;

    println!("{result}");

    Ok(())
}
