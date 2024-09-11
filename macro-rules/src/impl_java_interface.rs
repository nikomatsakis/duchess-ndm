#[macro_export]
macro_rules! setup_impl_java_interface {
    (
        shim_name: $shim_name:tt,
        interface_type: $InterfaceType:ty,
        rust_type: $RustType:ty,
        human_class_name: $human_class_name:expr,
        jni_class_name: $jni_class_name:expr,
        trait_method_definitions: ($($trait_method_definitions:tt)*),
        impl_method_definitions: ($($impl_method_definitions:tt)*),
        method_signatures: ($($method_name:ident($($method_input:ident : $method_input_ty:ty),*) -> $method_output_ty:ty,)*),
        method_descriptors: ($($method_descriptor:tt,)*),
        hygiene: (
            $ShimClass:ident,
            $DEFINITION:ident,
            $JVM_OP:ident,
            $Arc:ident,
            $OnceCell:ident,
            $DummyTrait:ident,
        )
    ) => {
        const _: () = {
            use std::sync::Arc as $Arc;

            // Define a dummy trait `$DummyTrait` to host the method definitions
            // listed from the user's impl. Using a trait is needed because the methods
            // will refer to `&self` and so forth and so they need to be true methods.

            pub trait $DummyTrait {
                $($trait_method_definitions)*
            }

            impl $DummyTrait for $ShimClass {
                $($impl_method_definitions)*
            }

            // Define the shim class and implement the `ShimClassDefinition` trait for it.

            pub struct $ShimClass;

            impl duchess::ShimClassDefinition for $ShimClass {
                const HUMAN_NAME: &'static str = $human_class_name;

                fn class_cell() -> &'static once_cell::sync::OnceCell<duchess::java::lang::Class> {
                    static CELL: once_cell::sync::OnceCell<duchess::java::lang::Class> =
                        once_cell::sync::OnceCell::new();
                    &CELL
                }

                fn constructor_cell() -> &'static once_cell::sync::OnceCell<duchess::raw::MethodPtr>
                {
                    static CELL: once_cell::sync::OnceCell<duchess::raw::MethodPtr> =
                        once_cell::sync::OnceCell::new();
                    &CELL
                }

                fn class<'jvm>(
                    jvm: &mut duchess::Jvm<'jvm>,
                ) -> duchess::LocalResult<'jvm, duchess::Local<'jvm, duchess::java::lang::Class>>
                {
                    duchess::plumbing::find_class(jvm, $jni_class_name)
                }

                fn java_functions() -> Vec<duchess::JavaFunction> {
                    $(
                        extern "C" fn $method_name(
                            env: duchess::plumbing::EnvPtr<'_>,
                            _class: duchess::plumbing::jni_sys::jclass
                            $($method_input: $method_input_ty,)*
                            native_pointer: i64,
                        ) {
                            let native_pointer = unsafe { &*(native_pointer as *const $RustType) };
                            duchess::native_function_returning_object(
                                env,
                                || {

                                }
                            )
                        }
                    )
                    unsafe {
                        vec![
                            $(
                                duchess::JavaFunction::new(
                                    stringify!($method_name),
                                    $method_descriptor,
                                    std::ptr::NonNull::new_unchecked( as *mut ()),
                                    Self::class,
                                ),
                            )*
                        ]
                    }
                }
            }

            // Define the JvmOp type that will convert an instance of `$RustType` into an instance of the shim type.
            // This can be cast to a pointer to the Java interface since the shim implements the interface.

            impl duchess::IntoJava<$InterfaceType> for $RustType {
                type JvmOp = $JVM_OP;

                // To start we just store the Rust value
                fn into_op(self) -> Self::JvmOp {
                    $JVM_OP { rust_value: self }
                }
            }

            #[derive(Clone)]
            pub struct $JVM_OP {
                rust_value: $RustType,
            }

            impl duchess::JvmOp for $JVM_OP {
                type Output<'jvm> = duchess::Local<'jvm, $InterfaceType>;

                fn do_jni<'jvm>(
                    self,
                    jvm: &mut duchess::Jvm<'jvm>,
                ) -> duchess::LocalResult<'jvm, Self::Output<'jvm>> {
                    unsafe {
                        $ShimClass::instantiate_shim_for::<_, $InterfaceType>(jvm, self.rust_value)
                    }
                }
            }
        };
    };
}
