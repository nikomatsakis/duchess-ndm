use std::error::Error;

use noak::{
    writer::{attributes::code::InstructionWriter, EncoderContext, FieldRef, MethodRef},
    AccessFlags, Version,
};
use proc_macro2::Span;
use syn::spanned::Spanned;

use crate::
    class_info::{
        ClassInfo,
        ClassKind::{Class, Interface},
        DotId, Id, Method, ScalarType, Type,
    }
;

/// Given the name of a Java interface, returns the bytecode for
/// a class that implements this interface. The class will have a constructor
/// with a single argument of type `long` that should be a pointer to a `Box<T>`.
/// Each method `m` in the interface will invoke a native method `native$m` that
/// indirects to Rust.
pub fn generate_interface_shim(interface: &ClassInfo) -> syn::Result<(DotId, Vec<u8>)> {
    match interface.kind {
        Class => {
            return Err(syn::Error::new(
                Span::call_site(),
                format!(
                    "cannot generate interface shim for class `{}`",
                    interface.name
                ),
            ));
        }

        Interface => { /* OK */ }
    }

    if let Some(first) = interface.extends.first() {
        return Err(syn::Error::new(
            Span::call_site(),
            format!(
                "cannot generate interface shim for class `{}` because it extends `{}`",
                interface.name, first,
            ),
        ));
    }

    for method in &interface.methods {
        if !method.generics.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                format!(
                    "cannot generate interface shim for class `{}` because method `{}` has generics", 
                    interface.name,
                    method.name,
                ),
            ));
        }
    }

    let class_name = DotId::duchess().dot(&format!("Impl${}", interface.name.to_dollar_name()));
    let native_pointer_field = ("nativePointer", ScalarType::Long.descriptor());
    let build_class = || -> Result<Vec<u8>, noak::error::EncodeError> {
        noak::writer::ClassWriter::new()
            .version(Version::V15)?
            .access_flags(AccessFlags::STATIC | AccessFlags::FINAL)?
            .this_class(&**class_name.class_name())?
            .super_class(&*DotId::object().to_jni_name())?
            .interfaces(|b| {
                b.begin(|w| w.interface(&*interface.name.to_jni_name()))?;
                Ok(())
            })?
            .fields(|b| {
                b.begin(|w| {
                    w.access_flags(AccessFlags::empty())?
                        .name("nativePointer")?
                        .descriptor(ScalarType::Long.descriptor())? // long
                        .attributes(|_w| Ok(()))
                })?;
                Ok(())
            })?
            .methods(|b| {
                b.begin(|w| {
                    // Generate the constructor
                    //
                    //     Dummy(long nativePointer) {
                    //         this.nativePointer = nativePointer;
                    //     }
                    w.access_flags(AccessFlags::empty())?
                        .name("<init>")?
                        .descriptor(&*Method::descriptor_from_types(
                            &[ScalarType::Long.into()],
                            &None,
                        ))?
                        .attributes(|w| {
                            w.begin(|w| {
                                w.code(|w| {
                                    w.max_stack(1)?
                                        .max_locals(2)?
                                        .instructions(|w| {
                                            w.aload0()?.invokespecial(MethodRef::by(
                                                "java/lang/Object",
                                                ("<init>", "()V"),
                                            ))?;
                                            w.aload0()?.lload1()?.putfield(FieldRef::by(
                                                &**class_name.class_name(),
                                                native_pointer_field,
                                            ))?;
                                            Ok(())
                                        })?
                                        .exceptions(|_| Ok(()))?
                                        .attributes(|_| Ok(()))
                                })
                            })?;
                            Ok(())
                        })
                })?;

                for method in &interface.methods {
                    // Given a method like
                    //
                    //    Type method(Arg1 arg1, Arg2 arg2)

                    b.begin(|w| {
                        // Generate a "native version" of the method
                        //
                        //    native Type method(long dataPointer, Arg1 arg1, Arg2 arg2)
                        let mut native_method = method.clone();
                        native_method.name = Id::from(format!("native${}", method.name));
                        native_method.argument_tys.push(ScalarType::Long.into());
                        w.access_flags(AccessFlags::NATIVE)?
                            .name(&*native_method.name)?
                            .descriptor(&*native_method.descriptor())?
                            .attributes(|_b| Ok(()))
                    })?
                    .begin(|w| {
                        // Generate the method itself
                        //
                        //    native Type method(long dataPointer, Arg1 arg1, Arg2 arg2)
                        let num_args = u16::try_from(method.argument_tys.len()).unwrap();
                        w.access_flags(AccessFlags::empty())?
                            .name(&*method.name)?
                            .descriptor(&*method.descriptor())?
                            .attributes(|w| {
                                w.begin(|w| {
                                    w.code(|w| {
                                        w.max_stack(num_args + 2)?
                                            .max_locals(num_args + 1)?
                                            .instructions(|w| {
                                                w.aload0()?; // load the this pointer

                                                for (argument_ty, i) in
                                                    method.argument_tys.iter().zip(0..)
                                                {
                                                    w.load(argument_ty, i)?;
                                                }

                                                w.aload0()?; // load the this pointer
                                                w.getfield(FieldRef::by(
                                                    &*class_name.to_jni_name(),
                                                    native_pointer_field,
                                                ))?;

                                                Ok(())
                                            })?
                                            .exceptions(|_| Ok(()))?
                                            .attributes(|_| Ok(()))
                                    })
                                })?;
                                Ok(())
                            })
                    })?;
                }
                Ok(())
            })?
            .attributes(|_b| Ok(()))?
            .into_bytes()
    };
    let bytes = build_class().map_err(ErrorAt::span(interface.span))?;

    Ok((class_name, bytes))
}

trait ExtTrait {
    fn load(&mut self, ty: &Type, i: u8) -> Result<&mut Self, noak::error::EncodeError>;
    fn iloadi(&mut self, i: u8) -> Result<&mut Self, noak::error::EncodeError>;
    fn aloadi(&mut self, i: u8) -> Result<&mut Self, noak::error::EncodeError>;
}

impl<CW> ExtTrait for InstructionWriter<CW>
where
    CW: EncoderContext,
{
    fn load(&mut self, ty: &Type, i: u8) -> Result<&mut Self, noak::error::EncodeError> {
        match ty {
            Type::Ref(_) => self.aloadi(i),
            Type::Scalar(s) => match s {
                ScalarType::Long => self.lload(i),

                // The JVM stores most 'small int types' as ints.
                ScalarType::Int
                | ScalarType::Byte
                | ScalarType::Short
                | ScalarType::Boolean
                | ScalarType::Char => self.iloadi(i),

                ScalarType::F64 => self.dload(i),
                ScalarType::F32 => self.fload(i),
            },
            Type::Repeat(_) => self.aloadi(i),
        }
    }

    fn iloadi(&mut self, i: u8) -> Result<&mut Self, noak::error::EncodeError> {
        match i {
            0 => self.iload0(),
            1 => self.iload1(),
            2 => self.iload2(),
            3 => self.iload3(),
            _ => self.iload(i),
        }
    }

    fn aloadi(&mut self, i: u8) -> Result<&mut Self, noak::error::EncodeError> {
        match i {
            0 => self.aload0(),
            1 => self.aload1(),
            2 => self.aload2(),
            3 => self.aload3(),
            _ => self.aload(i),
        }
    }
}

struct ErrorAt<E: Error> {
    span: Span,
    error: E,
}

impl<E> ErrorAt<E>
where
    E: Error,
{
    fn span(span: impl Spanned) -> impl FnOnce(E) -> Self {
        move |error| ErrorAt {
            span: span.span(),
            error,
        }
    }
}

impl<E: Error> From<ErrorAt<E>> for syn::Error {
    fn from(value: ErrorAt<E>) -> syn::Error {
        syn::Error::new(value.span, value.error)
    }
}
