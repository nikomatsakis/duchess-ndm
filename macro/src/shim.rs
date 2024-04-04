use std::error::Error;

use noak::{
    writer::{attributes::code::InstructionWriter, ClassWriter},
    AccessFlags, Version,
};
use proc_macro2::Span;
use syn::spanned::Spanned;

use crate::class_info::{
    ClassDecl, ClassInfo,
    ClassKind::{Class, Interface},
    DotId, Id, ScalarType, Type,
};

fn generate_interface_shim(class: &ClassInfo) -> syn::Result<()> {
    match class.kind {
        Class => {
            return Err(syn::Error::new(
                Span::call_site(),
                format!("cannot generate interface shim for class `{}`", class.name),
            ));
        }

        Interface => { /* OK */ }
    }

    if let Some(first) = class.extends.first() {
        return Err(syn::Error::new(
            Span::call_site(),
            format!(
                "cannot generate interface shim for class `{}` because it extends `{}`",
                class.name, first,
            ),
        ));
    }

    let build_class = || -> Result<_, noak::error::EncodeError> {
        noak::writer::ClassWriter::new()
            .version(Version::V15)?
            .access_flags(AccessFlags::STATIC | AccessFlags::FINAL)?
            .this_class(&*format!("Dummy"))?
            .super_class(&*DotId::object().to_jni_name())?
            .interfaces(|b| {
                b.begin(|w| w.interface(&*class.name.to_jni_name()))?;
                Ok(())
            })?
            .fields(|b| {
                b.begin(|w| {
                    w.access_flags(AccessFlags::empty())?
                        .name("nativePointer")?
                        .descriptor("J")? // long
                        .attributes(|w| Ok(()))
                })?;
                Ok(())
            })?
            .methods(|b| {
                for method in &class.methods {
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
                            .attributes(|w| Ok(()))
                    })?
                    .begin(|w| {
                        // Generate the method itself
                        //
                        //    native Type method(long dataPointer, Arg1 arg1, Arg2 arg2)
                        w.access_flags(AccessFlags::empty())?
                            .name(&*method.name)?
                            .descriptor(&*method.descriptor())?
                            .attributes(|w| {
                                w.begin(|w| {
                                    w.code(|w| {
                                        w.max_stack(6)?
                                            .max_locals(2)?
                                            .instructions(|w| {
                                                for (argument_ty, i) in
                                                    method.argument_tys.iter().zip(0..)
                                                {
                                                    w.load(argument_ty, i)?;
                                                }
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
            .attributes(|b| Ok(()))?
            .finish()
    };
    let builder = build_class().map_err(ErrorAt::span(class.span))?;

    for method in &class.methods {
        if !method.generics.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                format!(
                    "cannot generate interface shim for class `{}` because method `{}` has generics", 
                    class.name,
                    method.name,
                ),
            ));
        }
    }

    Ok(())
}

trait ExtTrait {
    fn load(&mut self, ty: Type, i: u8) -> Result<(), noak::error::EncodeError>;
}

impl<State> ExtTrait for InstructionWriter<ClassWriter<State>>
where

{
    fn load(&mut self, ty: Type, i: u8) -> Result<(), noak::error::EncodeError> {
        match ty {
            Type::Ref(_) => self.aload(i)?,
            Type::Scalar(s) => match s {
                ScalarType::Int => todo!(),
                ScalarType::Long => todo!(),
                ScalarType::Short => todo!(),
                ScalarType::Byte => todo!(),
                ScalarType::F64 => todo!(),
                ScalarType::F32 => todo!(),
                ScalarType::Boolean => todo!(),
                ScalarType::Char => todo!(),
            },
            Type::Repeat(_) => self.aload(i)?,
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
