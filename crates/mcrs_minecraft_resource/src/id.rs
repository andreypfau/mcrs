use alloc::borrow::ToOwned;
use alloc::string::String;
use core::any::type_name;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

macro_rules! dense_id {
    ($name:ident) => {
        impl<R> $name<R> {
            pub const fn checked(number: u16, len: usize) -> Option<Self> {
                if (number as usize) < len {
                    Some(Self {
                        number,
                        registry: PhantomData,
                    })
                } else {
                    None
                }
            }

            pub const fn number(self) -> u16 {
                self.number
            }

            pub const fn index(self) -> usize {
                self.number as usize
            }
        }

        impl<R> Clone for $name<R> {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<R> Copy for $name<R> {}

        impl<R> PartialEq for $name<R> {
            fn eq(&self, other: &Self) -> bool {
                self.number == other.number
            }
        }

        impl<R> Eq for $name<R> {}

        impl<R> PartialOrd for $name<R> {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl<R> Ord for $name<R> {
            fn cmp(&self, other: &Self) -> Ordering {
                self.number.cmp(&other.number)
            }
        }

        impl<R> Hash for $name<R> {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.number.hash(state);
            }
        }

        impl<R> fmt::Debug for $name<R> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.number)
            }
        }
    };
}

pub struct Id<R> {
    number: u16,
    registry: PhantomData<fn() -> R>,
}

pub struct TagId<R> {
    number: u16,
    registry: PhantomData<fn() -> R>,
}

dense_id!(Id);
dense_id!(TagId);

impl<R> Id<R> {
    pub fn narrow<N: TryFrom<u16>>(self) -> Result<N, NarrowError> {
        N::try_from(self.number).map_err(|_| NarrowError {
            registry: type_name::<R>().to_owned(),
            id: self.number,
            bits: (size_of::<N>() * 8) as u32,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NarrowError {
    pub registry: String,
    pub id: u16,
    pub bits: u32,
}

impl fmt::Display for NarrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "id {} of registry {} does not fit in {} bits",
            self.id, self.registry, self.bits
        )
    }
}

impl core::error::Error for NarrowError {}
