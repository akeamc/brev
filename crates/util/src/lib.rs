pub use bitflags;

#[macro_export(local_inner_macros)]
macro_rules! flags {
    (
        $(#[$outer:meta])*
        $vis:vis $name:ident: $T:ty {
            $(
                $(#[$inner:ident $($args:tt)*])*
                ($value:expr, $serialized:literal, $flag:ident);
            )*
        }
    ) => {
        $crate::bitflags::bitflags! {
            #[derive(Debug, PartialEq, Eq)]
            $(#[$outer])*
            $vis struct $name: $T {
                $(
                    $(#[$inner $($args)*])*
                    const $flag = $value;
                )*
            }
        }

        impl $name {
            /// Returns an iterator over the names of the flags that are set.
            pub fn names(&self) -> impl Iterator<Item = &'static str> {
                self.iter().map(|f| match f {
                    $(
                        Self::$flag => $serialized,
                    )*
                    _ => ::std::unreachable!(),
                })
            }

            /// Returns a new set of flags from the given names. Unknown
            /// names are silently ignored.
            pub fn from_names(names: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
                names.into_iter().collect()
            }
        }

        impl<S: AsRef<str>> FromIterator<S> for $name {
            fn from_iter<T: IntoIterator<Item = S>>(iter: T) -> Self {
                iter.into_iter().fold(Self::empty(), |flags, name| flags | match name.as_ref() {
                    $(
                        $serialized => Self::$flag,
                    )*
                    _ => Self::empty(),
                })
            }
        }
    }
}
