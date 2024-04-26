use std::sync::Arc;

use proc_macro2::{Literal, Span, TokenStream};
use quote::quote;
use syn::{spanned::Spanned, Generics};

use crate::{
    class_info::{ClassInfo, ClassRef, Method, ScalarType},
    java_function,
    reflect::Reflector,
    signature::Signature,
};

mod shim;

/// See [`crate::impl_java_interface`][] for user docs.
///
/// For this example:
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
///
/// we will generate roughly the following
///
/// ```rust
/// const _: () = {
///     impl JvmOp for Buffer {
///                 // ------ the Rust type that is implementing the interface
///         type Output<'jvm> = Local<'jvm, java::lang::Readable>;
///                             //          -------------------- the java interface we are implementing
///         fn execute_with(jvm: &mut Jvm<'_>) -> duchess::Result<'_, > {
///             // Code to
///             // - lazilly create shim class definition
///             // - link the shim class methods (e.g., `read`) to the jvm
///             // - create and return the Java object owning the Rust `Buffer`
///         }
///     }
///
///     #[java_function] // <-- we don't iterally generate this, but we generate the expansion of it
///     fn read(&self, cb: &java::nio::CharBuffer) -> duchess::GlobalResult<i32> {
///         /*  */
///     }
/// };
/// ```
pub fn impl_java_interface(item_impl: &syn::ItemImpl) -> syn::Result<TokenStream> {
    let reflector = &mut Reflector::default();
    let mut java_interface = JavaInterface::new(item_impl, reflector)?;
    let jvmop_impl = java_interface.generate_jvmop_impl()?;

    Ok(quote! {
        const _: () = {
            #jvmop_impl
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
        // Generate the bytecode for a shim class that implements the interface via native methods.
        let (_shim_name, bytes) = shim::generate_interface_shim(&self.class)?;
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
            &None,
        ));

        Ok(quote!(
            #(#attrs)*
            #impl_token
                #lt_token
                #params
                #gt_token
            duchess::JvmOp for #self_ty
                #where_clause
            {
                type Output<'jvm> = duchess::Local<'jvm, #interface_ty>;

                fn execute_with<'jvm>(
                    self,
                    jvm: &mut duchess::Jvm<'jvm>,
                ) -> duchess::Result<'jvm, Self::Output<'jvm>> {
                    // Lazilly (and at most once) load the shim class definition into the JVM.
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

                    // Move `self` into an `Arc`. The `DeferDrop` holds a handle
                    // (converted to a raw pointer) to this `Arc`. If all goes well,
                    // ownership of that handle will transfer into the Java object.
                    struct DeferDrop {
                        value: *const #self_ty
                    }
                    impl DeferDrop {
                        fn free_arc(value: *const #self_ty) {
                            unsafe {
                                std::mem::drop(Arc::from_raw(self.value));
                            }
                        }
                    }
                    impl Drop for DeferDrop {
                        fn drop(&mut self) {
                            Self::free_arc(self.value)
                        }
                    }
                    let this = Arc::new(self);
                    let this = DeferDrop { value: Arc::into_raw(this) };

                    // Invoke the constructor of the shim class.
                    let env = jvm.env();
                    let obj: duchess::Result<'_, ::core::option::Option<duchess::Local<#interface_ty>>> = unsafe {
                        // The constructor expects one argument: the raw pointer to the arc as a Java long
                        let this_long = this_drop.value as usize as i64;

                        env.invoke(|env| env.NewObjectA, |env, f| f(
                            env,
                            duchess::plumbing::JavaObjectExt::as_raw(&*class_global).as_ptr(),
                            constructor.as_ptr(),
                            [
                                duchess::plumbing::IntoJniValue::into_jni_value(this_long),
                            ].as_ptr(),
                        ))
                    };

                    // Check the result.
                    match obj {
                        Ok(Some(v)) => {
                            // The Java object was successfully created, we can "forget" our handle
                            // to `this_drop` now. When the java object is collected by the GC, it will
                            // trigger a callback to free the code.
                            std::mem::forget(this_drop);
                            Ok(v)
                        }

                        Ok(None) => {
                            // NewObjectA should only return a null pointer when an exception occurred in the
                            // constructor, so reaching here is a strange JVM state
                            duchess::Error::JvmInternal(format!(
                                "failed to create new shim for `{}`",
                                #interface_jni,
                            ))
                        }

                        Err(err) => {
                            // Internal JVM error
                            Err(err)
                        }
                    }
                }
            }
        ))
    }

    /// generates the native methods for each item in the interface, e.g., roughly...
    ///
    /// ```rust
    /// #[java_function] // <-- we don't iterally generate this, but we generate the expansion of it
    /// fn read(&self, cb: &java::nio::CharBuffer) -> duchess::GlobalResult<i32> {
    ///     /*  */
    /// }
    /// ```
    fn native_methods(&mut self) -> syn::Result<TokenStream> {
        let mut output = TokenStream::default();
        for item in &self.item_impl.items {
            if let syn::ImplItem::Fn(syn::ImplItemFn {
                attrs,
                vis,
                defaultness,
                sig:
                    sig @ syn::Signature {
                        constness: None,
                        asyncness: None,
                        unsafety: None,
                        abi: None,
                        fn_token,
                        ident,
                        generics,
                        paren_token,
                        inputs,
                        variadic: None,
                        output,
                    },
                block,
            }) = item
            {
                self.check_method_sig(sig)?;

                java_function::java_function(
                    x,
                    syn::ItemFn {
                        attrs: attrs.clone(),
                        vis: syn::Visibility::Inherited,
                        sig: syn::Signature {
                            constness: None,
                            asyncness: None,
                            unsafety: None,
                            abi: None,
                            fn_token: fn_token.clone(),
                            ident: ident.clone(),
                            generics: generics.clone(),
                            paren_token: paren_token.clone(),
                            inputs: inputs.clone(),
                            variadic: None,
                            output: output.clone(),
                        },
                        block: block.clone(),
                    },
                );
            } else {
                return Err(syn::Error::new(
                    item.span(),
                    "unexpected impl item, only `fn` are permitted",
                ));
            }
        }
        Ok(output)
    }

    fn check_method_sig(&self, sig: &syn::Signature) -> syn::Result<()> {
        let syn::Signature {
            constness,
            asyncness,
            unsafety,
            abi,
            fn_token,
            ident,
            generics,
            paren_token,
            inputs,
            variadic,
            output,
        } = sig;

        self.forbid_some(constness)?;
        self.forbid_some(asyncness)?;
        self.forbid_some(unsafety)?;
        self.forbid_some(abi)?;
        self.forbid_some(variadic)?;

        Ok(())
    }

    fn forbid_some<S>(&self, o: &Option<S>) -> syn::Result<()>
    where
        S: Spanned,
    {
        if let Some(o) = o {
            Err(syn::Error::new(o.span(), "unexpected declaration"))
        } else {
            Ok(())
        }
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
