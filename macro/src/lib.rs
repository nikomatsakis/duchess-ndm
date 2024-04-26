use argument::{DuchessDeclaration, JavaPath, MethodSelector};
use parse::Parser;
use proc_macro::TokenStream;
use rust_format::Formatter;

mod argument;
mod check;
mod class_info;
mod codegen;
mod derive;
mod impl_java_interface;
mod java_function;
mod parse;
mod reflect;
mod signature;
mod substitution;
mod upcasts;

/// The main duchess macro, used like so
///
/// ```rust,ignore
/// duchess::java_package! {
///     package some.pkg.name;
///     class SomeDotId { * }
/// }
/// ```
///
/// see the tutorial in the [duchess book] for more info.
///
/// [duchess book]: https://nikomatsakis.github.io/duchess/
#[proc_macro]
pub fn java_package(input: TokenStream) -> TokenStream {
    let input: proc_macro2::TokenStream = input.into();
    let decl = match Parser::from(input).parse::<DuchessDeclaration>() {
        Ok(decl) => decl,
        Err(err) => return err.to_compile_error().into(),
    };

    match decl.to_tokens() {
        Ok(t) => return t.into(),
        Err(e) => return e.into_compile_error().into(),
    }
}

#[proc_macro_attribute]
pub fn java_function(args: TokenStream, input: TokenStream) -> TokenStream {
    let args: proc_macro2::TokenStream = args.into();
    let args = match Parser::from(args).parse::<MethodSelector>() {
        Ok(decl) => decl,
        Err(err) => return err.to_compile_error().into(),
    };

    let item_fn = match syn::parse::<syn::ItemFn>(input) {
        Ok(item_fn) => item_fn,
        Err(err) => return err.into_compile_error().into(),
    };

    match java_function::java_function(args, item_fn) {
        Ok(t) => t.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

/// Implement a Java interface for a Rust type.
///
/// # Example
///
/// Here is an example of implement the `java::lang::Readable`
/// trait for a Rust type `Buffer`.
///
///
/// ```rust
/// struct Buffer {
///     chars: Vec<Char>
/// }
///
/// #[impl_java_interface]
/// impl java::lang::Readable for Buffer {
///     fn read(&self, cb: &java::nio::CharBuffer) -> duchess::GlobalResult<i32> {
///         for &ch in &self.chars {
///             cb.put(ch).execute();
///         }
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn impl_java_interface(args: TokenStream, input: TokenStream) -> TokenStream {
    let args: proc_macro2::TokenStream = args.into();
    let no_args = match args.clone().into_iter().next() {
        Some(arg) => Err(syn::Error::new(arg.span(), "no arguments expected")),
        None => Ok(()),
    };

    let result = no_args.and_then(|()| {
        let item_impl = syn::parse::<syn::ItemImpl>(input)?;
        impl_java_interface::impl_java_interface(&item_impl)
    });

    match result {
        Ok(t) => {
            debug_tokens("impl_java_interface", &t);
            t.into()
        }
        Err(err) => err.into_compile_error().into(),
    }
}

synstructure::decl_derive!([ToRust, attributes(java)] => derive::derive_to_rust);

synstructure::decl_derive!([ToJava, attributes(java)] => derive::derive_to_java);

fn debug_tokens(name: impl std::fmt::Display, token_stream: &proc_macro2::TokenStream) {
    let Ok(f) = std::env::var("DUCHESS_DEBUG") else {
        return;
    };
    if f == "*" || f == "1" || name.to_string().starts_with(&f) {
        match rust_format::RustFmt::default().format_tokens(token_stream.clone()) {
            Ok(v) => {
                eprintln!("{v}");
            }
            Err(_) => {
                eprintln!("{token_stream:?}");
            }
        }
    }
}
