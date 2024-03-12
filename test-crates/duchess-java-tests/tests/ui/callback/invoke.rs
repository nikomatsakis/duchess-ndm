use duchess::prelude::*;

duchess::java_package! {
    package callback;

    class Callback { * }
    class CallCallback { * }
    class Dummy { * }
}

struct Callback {
    last_name: String,
    age: u32,
}

impl Callback {
    fn name(&self, input: &str) -> String {
        format!("{} {}", input, self.last_name)
    }

    fn age(&self) -> u32 {
        self.age
    }
}

#[duchess::java_function(callback.Dummy::getNameNative)]
fn get_name_native(
    _this: &callback::Dummy,
    native_pointer: u64,
    input: String,
) -> duchess::GlobalResult<String> {
    let callback: &Callback = unsafe { &*(native_pointer as usize as *const Callback) };
    Ok(format!("{name} {}", callback.last_name))
}

fn main() -> duchess::GlobalResult<()> {
    duchess::Jvm::builder()
        .link(vec![get_name_native::java_fn()])
        .try_launch()?;

    let ccb = callback::CallCallback::new().execute()?;

    // wrap the Rust box in an instance of `Dummy`
    let value = Box::new(Callback {
        last_name: "Rustacean".to_string(),
        age: 22,
    });
    let value_long: u64 = value.as_ptr() as usize as u64;
    let arg = callback::Dummy::new(value_long).global().execute()?;

    let result: String = ccb.method(&arg)?.to_rust().execute();

    println!("{result}");

    Ok(())
}
