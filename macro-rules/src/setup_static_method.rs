#[macro_export]
macro_rules! setup_static_method {
    (
        struct_name: [$S:ident],
        java_class_generics: [$($G:ident,)*],
        rust_method_name: [$M:ident],
        rust_method_generics: [$($MG:ident,)*],
        input_names: [$($I:tt,)*],
        input_ty_tts: [$($I_ty:tt,)*],
        input_ty_ops: [$($I_op:path,)*],
        output_ty_tt: [$O_ty:tt],
        sig_where_clauses: [$($SIG:tt)*],
        prepare_inputs: [$($prepare_inputs:tt)*],
        jni_call_fn: [$jni_call_fn:ident],
        jni_method: [$jni_method:expr],
        jni_descriptor: [$jni_descriptor:expr],
        idents: [$this:ident, $jvm:ident],
    ) => {
        pub fn $M<$($MG,)*>(
            $($I: duchess::plumbing::argument_impl_trait!($I_ty),)*
        ) -> duchess::plumbing::output_trait!($O_ty)
        where
            $($SIG)*
        {
            // Create a struct that will implement the `JvmOp`.
            pub struct $M<
                $($G,)*
                $($MG,)*
                $($I,)*
            > {
                $($I : $I,)*
                phantom: ::core::marker::PhantomData<($($G,)* $($MG,)* $($I,)*)>,
            }

            impl<$($G,)* $($MG,)* $($I,)*> ::core::clone::Clone
            for $M<$($G,)* $($MG,)* $($I,)*>
            where
                $($I: $I_op,)*
                $($G: duchess::JavaObject,)*
                $($SIG)*
            {
                fn clone(&self) -> Self {
                    $M {
                        $($I: Clone::clone(&self.$I),)*
                        phantom: self.phantom,
                    }
                }
            }

            impl<$($G,)* $($MG,)* $($I,)*> duchess::prelude::JvmOp
            for $M<$($G,)* $($MG,)* $($I,)*>
            where
                $($I: $I_op,)*
                $($G: duchess::JavaObject,)*
                $($SIG)*
            {
                type Output<'jvm> = duchess::plumbing::output_type!('jvm, $O_ty);

                fn do_jni<'jvm>(
                    $this,
                    $jvm: &mut duchess::Jvm<'jvm>,
                ) -> duchess::LocalResult<'jvm, Self::Output<'jvm>> {
                    use duchess::plumbing::once_cell::sync::OnceCell;

                    $($prepare_inputs)*

                    // Cache the method id for this method -- note that we only have one cache
                    // no matter how many generic monomorphizations there are. This makes sense
                    // given Java's erased-based generics system.
                    static METHOD: OnceCell<duchess::plumbing::MethodPtr> = OnceCell::new();
                    let method = METHOD.get_or_try_init(|| {
                        let class = <$S<$($G,)*> as duchess::JavaObject>::class($jvm)?;
                        duchess::plumbing::find_method($jvm, &class, $jni_method, $jni_descriptor, true)
                    })?;

                    let class = <$S<$($G,)*> as duchess::JavaObject>::class($jvm)?;
                    unsafe {
                        $jvm.env().invoke(|env| env.$jni_call_fn, |env, f| f(
                            env,
                            duchess::plumbing::JavaObjectExt::as_raw(&*class).as_ptr(),
                            method.as_ptr(),
                            [
                                $(duchess::plumbing::IntoJniValue::into_jni_value($I),)*
                            ].as_ptr(),
                        ))
                    }
                }
            }

            duchess::plumbing::macro_if! {
                if is_ref_ty($O_ty) {
                    impl<$($G,)* $($MG,)* $($I,)*> ::core::ops::Deref
                    for $M<$($G,)* $($MG,)* $($I,)*>
                    where
                        $($G: duchess::JavaObject,)*
                        $($SIG)*
                    {
                        type Target = duchess::plumbing::view_of_op!($O_ty);

                        fn deref(&self) -> &Self::Target {
                            <Self::Target as duchess::plumbing::FromRef<_>>::from_ref(self)
                        }
                    }
                }
            }

            $M {
                $($I: $I.into_op(),)*
                phantom: ::core::default::Default::default(),
            }
        }
    };
}