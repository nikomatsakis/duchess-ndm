use duchess_reflect::class_info::ClassRef;
use proc_macro2::{Span, TokenStream};
use syn::spanned::Spanned as _;

use crate::hygiene::Hygiene;

// Input:
//
// ```
// #[impl_java_interface]
// impl callback::GetName for $RustType {
//     ... methods ...
// }
// ```
//
// at build.rs time we generate Rust code with the bytecode for
//
// ```java
// package duchess;
// public class Shim$callback$GetName implements callback.GetName {
//     long nativePointer;
//     static Cleaner cleaner = Cleaner.create();
//     public Shim$callback$GetName(long nativePointer) {
//         this.nativePointer = nativePointer;
//         cleaner.register(this, () -> { native$drop(nativePointer); });
//     }
//     native static void native$drop(long nativePointer);
//     native static java.lang.String native$getName(
//         java.lang.String arg0,
//         long nativePointer
//     );
//     public java.lang.String getName(
//         java.lang.String arg0,
//         long nativePointer
//     ) {
//         return native$getName(
//             arg0,
//             nativePointer
//         );
//     }
// }
// ```
//
// which expands to
//
// ```
// const _: () = {
//     #[derive(Clone)]
//     pub struct ToJavaInterface {
//         cb: Arc<Callback>,
//     }
//
//     // Convert from $RustType into the Java interface
//     impl duchess::IntoJava<callback::GetName> for $RustType {
//         type JvmOp = ToJavaInterface;
//
//         fn into_op(self) -> Self::JvmOp {
//             ToJavaInterface { cb: Arc::new(self) }
//         }
//     }
//
//     impl duchess::JvmOp for ToJavaInterface {
//         type Output<'jvm> = duchess::Local<'jvm, ShimType>;
//
//         fn execute_with<'jvm>(
//             self,
//             jvm: &mut duchess::Jvm<'jvm>,
//         ) -> duchess::Result<'jvm, Self::Output<'jvm>> {
//             let value = self.cb.clone();
//             let value_long: i64 = Arc::into_raw(value) as usize as i64;
//             /* JNI Code to invoke the constructor */
//         }
//     }
// };
// ```

pub fn impl_java_interface(input: syn::ItemImpl) -> syn::Result<TokenStream> {
    let hygiene = Hygiene::from2(&input);
    ImplJavaInterface {
        hygiene,
        item: &input,
    }
    .generate()
}

struct ImplJavaInterface<'syn> {
    hygiene: Hygiene,
    item: &'syn syn::ItemImpl,
}

impl ImplJavaInterface<'_> {
    fn generate(self) -> syn::Result<TokenStream> {
        let (_class_ref, _span) = self.java_interface()?;
        Ok(TokenStream::new())
    }

    fn java_interface(&self) -> syn::Result<(ClassRef, Span)> {
        let Some((_, trait_path, _)) = &self.item.trait_ else {
            return Err(syn::Error::new_spanned(&self.item, "expected an impl of a trait").into());
        };
        let class_ref = ClassRef::from(&self.item.generics, trait_path)?;
        Ok((class_ref, trait_path.span()))
    }
}
