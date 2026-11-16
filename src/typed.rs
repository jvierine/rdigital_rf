//! Typed sample I/O. Integer requests require matching stored component types.
use crate::*;
use hdf5::types::{CompoundField, CompoundType, TypeDescriptor};
use hdf5::H5Type;
use std::mem::size_of;
mod sealed {
    pub trait Scalar {}
    pub trait Sample {}
}
pub trait Scalar: sealed::Scalar + H5Type + Copy + std::fmt::Debug + 'static {
    const FLOAT: bool;
    fn scalar_zero() -> Self;
    fn scalar_same(self, other: Self) -> bool;
    fn i8(v: i8) -> Self;
    fn u8(v: u8) -> Self;
    fn i16(v: i16) -> Self;
    fn u16(v: u16) -> Self;
    fn i32(v: i32) -> Self;
    fn u32(v: u32) -> Self;
    fn i64(v: i64) -> Self;
    fn u64(v: u64) -> Self;
    fn f32(v: f32) -> Self;
    fn f64(v: f64) -> Self;
    fn convert<D: Scalar>(self) -> D;
}
macro_rules! scalar {
    ($t:ty,$method:ident,$float:expr,$same:expr) => {
        impl sealed::Scalar for $t {}
        impl Scalar for $t {
            const FLOAT: bool = $float;
            #[inline]
            fn scalar_zero() -> Self {
                0 as Self
            }
            #[inline]
            fn scalar_same(self, other: Self) -> bool {
                ($same)(self, other)
            }
            #[inline]
            fn i8(v: i8) -> Self {
                v as Self
            }
            #[inline]
            fn u8(v: u8) -> Self {
                v as Self
            }
            #[inline]
            fn i16(v: i16) -> Self {
                v as Self
            }
            #[inline]
            fn u16(v: u16) -> Self {
                v as Self
            }
            #[inline]
            fn i32(v: i32) -> Self {
                v as Self
            }
            #[inline]
            fn u32(v: u32) -> Self {
                v as Self
            }
            #[inline]
            fn i64(v: i64) -> Self {
                v as Self
            }
            #[inline]
            fn u64(v: u64) -> Self {
                v as Self
            }
            #[inline]
            fn f32(v: f32) -> Self {
                v as Self
            }
            #[inline]
            fn f64(v: f64) -> Self {
                v as Self
            }
            #[inline]
            fn convert<D: Scalar>(self) -> D {
                D::$method(self)
            }
        }
    };
}
scalar!(i8, i8, false, |a: i8, b: i8| a == b);
scalar!(u8, u8, false, |a: u8, b: u8| a == b);
scalar!(i16, i16, false, |a: i16, b: i16| a == b);
scalar!(u16, u16, false, |a: u16, b: u16| a == b);
scalar!(i32, i32, false, |a: i32, b: i32| a == b);
scalar!(u32, u32, false, |a: u32, b: u32| a == b);
scalar!(i64, i64, false, |a: i64, b: i64| a == b);
scalar!(u64, u64, false, |a: u64, b: u64| a == b);
scalar!(f32, f32, true, |a: f32, b: f32| a.to_bits() == b.to_bits());
scalar!(f64, f64, true, |a: f64, b: f64| a.to_bits() == b.to_bits());
/// Interleaved complex samples, with stored component precision.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Iq<T> {
    pub r: T,
    pub i: T,
}
unsafe impl<T: Scalar> H5Type for Iq<T> {
    #[inline]
    fn type_descriptor() -> TypeDescriptor {
        TypeDescriptor::Compound(CompoundType {
            fields: vec![
                CompoundField::typed::<T>("r", 0, 0),
                CompoundField::typed::<T>("i", size_of::<T>(), 1),
            ],
            size: size_of::<Self>(),
        })
    }
}
/// Sealed POD sample types: primitives, `Iq<T>`, num_complex float32/float64 pairs.
pub trait Sample: sealed::Sample + Copy + std::fmt::Debug + 'static {
    type Component: Scalar;
    type Memory: H5Type + Copy;
    const COMPLEX: bool;
    fn pair(r: Self::Component, i: Self::Component) -> Self;
    fn memory(v: Self::Memory) -> Self;
    fn same(self, other: Self) -> bool;
    #[inline]
    fn zero() -> Self {
        Self::pair(
            Self::Component::scalar_zero(),
            Self::Component::scalar_zero(),
        )
    }
}
impl<T: Scalar> sealed::Sample for T {}
impl<T: Scalar> Sample for T {
    type Component = T;
    type Memory = T;
    const COMPLEX: bool = false;
    #[inline]
    fn pair(r: T, _: T) -> Self {
        r
    }
    #[inline]
    fn memory(v: T) -> Self {
        v
    }
    #[inline]
    fn same(self, o: Self) -> bool {
        Scalar::scalar_same(self, o)
    }
}
impl<T: Scalar> sealed::Sample for Iq<T> {}
impl<T: Scalar> Sample for Iq<T> {
    type Component = T;
    type Memory = Self;
    const COMPLEX: bool = true;
    #[inline]
    fn pair(r: T, i: T) -> Self {
        Self { r, i }
    }
    #[inline]
    fn memory(v: Self) -> Self {
        v
    }
    #[inline]
    fn same(self, o: Self) -> bool {
        self.r.scalar_same(o.r) && self.i.scalar_same(o.i)
    }
}
macro_rules! complex {
    ($t:ty,$c:ty) => {
        impl sealed::Sample for $t {}
        impl Sample for $t {
            type Component = $c;
            type Memory = Iq<$c>;
            const COMPLEX: bool = true;
            #[inline]
            fn pair(r: $c, i: $c) -> Self {
                Self::new(r, i)
            }
            #[inline]
            fn memory(v: Iq<$c>) -> Self {
                Self::new(v.r, v.i)
            }
            #[inline]
            fn same(self, o: Self) -> bool {
                self.re.to_bits() == o.re.to_bits() && self.im.to_bits() == o.im.to_bits()
            }
        }
    };
}
complex!(num_complex::Complex32, f32);
complex!(num_complex::Complex64, f64);
#[derive(Debug)]
pub struct TypedVector<T> {
    pub samples: Vec<T>,
    pub shape: Vec<usize>,
}
pub(crate) fn check<T: Sample>(component: &TypeDescriptor, complex: bool) -> Result<()> {
    ensure!(
        !complex || T::COMPLEX,
        "complex samples require a complex output type"
    );
    ensure!(
        T::Component::FLOAT || *component == T::Component::type_descriptor(),
        "integer reads require matching stored component type"
    );
    Ok(())
}
impl Cached {
    pub(crate) fn append<T: Sample>(
        &self,
        a: usize,
        b: usize,
        ch: usize,
        complex: bool,
        out: &mut Vec<T>,
    ) -> Result<()> {
        self.append_selection(a, b, Some(ch), complex, out)
    }
    pub(crate) fn append_selection<T: Sample>(
        &self,
        a: usize,
        b: usize,
        ch: Option<usize>,
        complex: bool,
        out: &mut Vec<T>,
    ) -> Result<()> {
        if let Some(raw) = &self.raw {
            return raw.append_typed(a, b, ch, out);
        }
        let data = self.data.as_ref().unwrap();
        // A one-channel dataset is a contiguous two-dimensional slab. Preserve
        // that rank through HDF5 rather than squeezing each selected row into
        // a rank-one memory selection (slow with older chunked HDF5 readers).
        let ch = if data.shape()[1] == 1 { None } else { ch };
        let desc = data.dtype()?.to_descriptor()?;
        let scalar = if complex {
            match &desc {
                TypeDescriptor::Compound(c) => {
                    ensure!(c.fields.len() == 2, "invalid complex storage");
                    let r = c
                        .fields
                        .iter()
                        .find(|f| f.name == "r")
                        .context("missing real component")?;
                    let i = c
                        .fields
                        .iter()
                        .find(|f| f.name == "i")
                        .context("missing imaginary component")?;
                    ensure!(r.ty == i.ty, "inconsistent components");
                    r.ty.clone()
                }
                _ => bail!("invalid complex dtype"),
            }
        } else {
            desc
        };
        check::<T>(&scalar, complex)?;
        if scalar != T::Component::type_descriptor() {
            macro_rules! convert {($($s:ty),*)=>{$(if scalar==<$s>::type_descriptor(){
                return append_converted::<T,$s>(data,a,b,ch,complex,out);
            })*};}
            convert!(i8, u8, i16, u16, i32, u32, i64, u64, f32, f64);
            bail!("unsupported sample dtype");
        }
        if complex {
            macro_rules! append {
                ($v:expr) => {
                    out.extend($v?.iter().copied().map(T::memory))
                };
            }
            if let Some(ch) = ch {
                append!(data.read_slice_1d::<T::Memory, _>((a..b, ch)));
            } else {
                append!(data.read_slice_2d::<T::Memory, _>((a..b, ..)));
            }
        } else {
            macro_rules! append {
                ($v:expr) => {
                    out.extend(
                        $v?.iter()
                            .copied()
                            .map(|r| T::pair(r, T::Component::scalar_zero())),
                    )
                };
            }
            if let Some(ch) = ch {
                append!(data.read_slice_1d::<T::Component, _>((a..b, ch)));
            } else {
                append!(data.read_slice_2d::<T::Component, _>((a..b, ..)));
            }
        }
        Ok(())
    }
}

#[inline]

fn append_converted<T: Sample, S: Scalar>(
    data: &Dataset,
    a: usize,
    b: usize,
    ch: Option<usize>,
    complex: bool,
    out: &mut Vec<T>,
) -> Result<()> {
    if complex {
        macro_rules! append {
            ($v:expr) => {
                out.extend(
                    $v?.iter().map(|z| {
                        T::pair(z.r.convert::<T::Component>(), z.i.convert::<T::Component>())
                    }),
                )
            };
        }
        if let Some(ch) = ch {
            append!(data.read_slice_1d::<Iq<S>, _>((a..b, ch)));
        } else {
            append!(data.read_slice_2d::<Iq<S>, _>((a..b, ..)));
        }
    } else {
        macro_rules! append {
            ($v:expr) => {
                out.extend(
                    $v?.iter().map(|&r| {
                        T::pair(r.convert::<T::Component>(), T::Component::scalar_zero())
                    }),
                )
            };
        }
        if let Some(ch) = ch {
            append!(data.read_slice_1d::<S, _>((a..b, ch)));
        } else {
            append!(data.read_slice_2d::<S, _>((a..b, ..)));
        }
    }
    Ok(())
}
