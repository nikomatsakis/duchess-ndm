use std::sync::Arc;

use proc_macro2::{Literal, TokenStream};
use quote::quote;
use syn::{spanned::Spanned, Generics};

use crate::{
    argument::JavaPath,
    class_info::{ClassInfo, ClassRef, Method, Type},
    reflect::Reflector,
    signature::Signature,
};

mod shim;

pub fn impl_java_interface(item_impl: &syn::ItemImpl) -> syn::Result<TokenStream> {
    let reflector = &mut Reflector::default();
    let java_interface = JavaInterface::new(item_impl, &mut reflector)?;
    Ok(quote! {
        const _: () = {
        };
    })
}

struct JavaInterface<'me> {
    span: Span,
    java_interface: ClassRef,
    item_impl: &'me syn::ItemImpl,
    reflector: &'me mut Reflector,
    class: Arc<ClassInfo>,
}

impl<'me> JavaInterface<'me> {
    fn new(item_impl: &'me syn::ItemImpl, reflector: &'me mut Reflector) -> syn::Result<Self> {
        let syn::ItemImpl {
            attrs: _,
            defaultness,
            unsafety,
            impl_token,
            generics,
            trait_,
            self_ty: _,
            brace_token: _,
            items: _,
        } = item_impl;

        if let Some(d) = defaultness {
            return Err(syn::Error::new(d.span(), "default impls not supported"));
        }

        if let Some(d) = unsafety {
            return Err(syn::Error::new(d.span(), "unsafe impls not supported"));
        }

        let Some((None, trait_name, _for_token)) = trait_ else {
            return Err(syn::Error::new(
                item_impl.impl_token.span(),
                "impl must be of the form `impl $JavaInterface for $ty`",
            ));
        };

        let java_interface = ClassRef::from(generics, trait_name)?;
        if !java_interface.generics.is_empty() {
            return Err(syn::Error::new(
                trait_name.span(),
                "generic java interfaces are not presently supported",
            ));
        }

        let class = reflector.reflect(&java_interface.name, trait_name.span())?;

        Ok(Self {
            span: impl_token.span(),
            java_interface,
            item_impl,
            reflector,
            class,
        })
    }

    fn generate_jvmop_impl(&mut self) -> syn::Result<TokenStream> {
        let bytes = shim::generate_interface_shim(&self.class)?;
        let byte_literals: Vec<_> = bytes.iter().map(|b| Literal::u8_unsuffixed(*b)).collect();

        let syn::ItemImpl {
            attrs,
            defaultness: _,
            unsafety: _,
            impl_token,
            generics:
                Generics {
                    lt_token,
                    params,
                    gt_token,
                    where_clause,
                },
            trait_: _,
            self_ty,
            brace_token: _,
            items: _,
        } = self.item_impl;

        let mut sig = Signature::new(
            &self.class.name.class_name(),
            self.span,
            &self.class.generics,
        );
        let interface_ty = sig.class_ref_ty(&self.java_interface)?;
        let interface_jni = Literal::string(&self.java_interface.name.to_jni_name());

        let ctor_descriptor = Literal::string(&Method::descriptor_from_types(
            &[ScalarType::Long.into()],
            None,
        ));

        Ok(quote!(
            #(#attrs)*
            #impl_token
                #lt_token
                #params
                #gt_token
            duchess::JvmOp for $self_ty
                #where_clause
            {
                type Output<'jvm> = duchess::Local<'jvm, #interface_ty>;

                fn execute_with<'jvm>(
                    self,
                    jvm: &mut duchess::Jvm<'jvm>,
                ) -> duchess::Result<'jvm, Self::Output<'jvm>> {
                    const CLASS_DEFINITION: duchess::ClassDefinition = duchess::ClassDefinition::new_const(
                        #interface_jni,
                        &[#(#byte_literals,)*]
                    );

                    static CLASS: duchess::plumbing::once_cell::sync::OnceCell<duchess::Global<java::lang::Class>> = duchess::plumbing::once_cell::sync::OnceCell::new();
                    let class_global = CLASS.get_or_try_init::<_, duchess::Error<duchess::Local<java::lang::Throwable>>>(|| {
                        let class = jvm.define_class(&CLASS_DEFINITION)?;
                        Ok(jvm.global(&class))
                    })?;

                    static CONSTRUCTOR: duchess::plumbing::once_cell::sync::OnceCell<duchess::plumbing::MethodPtr> = duchess::plumbing::once_cell::sync::OnceCell::new();
                    let constructor = CONSTRUCTOR.get_or_try_init(|| {
                        let ctor_descriptor = CString::new(#ctor_descriptor).unwrap();
                        duchess::plumbing::find_constructor(jvm, &class_global, &ctor_descriptor)
                    })?;

                    let env = jvm.env();
                    let obj: ::core::option::Option<duchess::Local<#ty>> = unsafe {
                        env.invoke(|env| env.NewObjectA, |env, f| f(
                            env,
                            duchess::plumbing::JavaObjectExt::as_raw(&*class).as_ptr(),
                            constructor.as_ptr(),
                            [
                                #(duchess::plumbing::IntoJniValue::into_jni_value(#input_names),)*
                            ].as_ptr(),
                        ))
                    }?;
                    obj.ok_or_else(|| {
                        // NewObjectA should only return a null pointer when an exception occurred in the
                        // constructor, so reaching here is a strange JVM state
                        duchess::Error::JvmInternal(format!(
                            "failed to create new `{}` via constructor `{}`",
                            #name, #descriptor,
                        ))
                    })

                    let value = self.cb.clone();
                    let value_long: i64 = Arc::into_raw(value) as usize as i64;
                    callback::Dummy::new(value_long).execute_with(jvm)
                }
            }
        ))
    }

    // #[duchess::java_function(callback.Dummy::getNameNative)]
    // fn get_name_native(
    //     _this: &callback::Dummy,
    //     name: &java::lang::String,
    //     native_pointer: i64,
    // ) -> duchess::GlobalResult<String> {
    //     let native_pointer: *mut Callback = native_pointer as usize as *mut Callback;
    //     let callback = unsafe { &*native_pointer };
    //     let name: String = name.to_rust().execute()?;
    //     Ok(format!("{name} {}", callback.last_name))
    // }

    // #[duchess::java_function(callback.Dummy::drop)]
    // fn drop_native(native_pointer: i64) -> () {
    //     let native_pointer: *mut Callback = native_pointer as usize as *mut Callback;
    //     unsafe {
    //         Arc::from_raw(native_pointer);
    //     }
    // }
}
