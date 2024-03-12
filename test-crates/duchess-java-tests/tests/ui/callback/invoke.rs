//@run

use duchess::java;
use duchess::prelude::*;

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

#[duchess::java_function(callback.Dummy::getNameNative)]
fn get_name_native(
    _this: &callback::Dummy,
    native_pointer: i64,
    name: &java::lang::String,
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
        Box::from_raw(native_pointer);
    }
}

fn main() -> duchess::GlobalResult<()> {
    duchess::Jvm::builder()
        .link(vec![get_name_native::java_fn()])
        .try_launch()?;

    let ccb = callback::CallCallback::new().global().execute()?;

    // wrap the Rust box in an instance of `Dummy`
    let value = Box::new(Callback {
        last_name: "Rustacean".to_string(),
    });
    let value_long: i64 = Box::into_raw(value) as usize as i64;
    let arg = callback::Dummy::new(value_long).global().execute()?;

    let result: String = ccb.method(&arg).assert_not_null().to_rust().execute()?;

    println!("{result}");

    Ok(())
}
