// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
macro_rules! make_error_system {
    (
        $error_kind:ident =>
        $error_name:ident =>
        $result_name:ident
        { $( $error:ident ),* }
    ) => {
        #[derive(Debug)]
        pub enum $error_kind {
            $( $error ),*
        }

        #[derive(Debug)]
        pub struct $error_name {
            pub kind: $error_kind,
            pub detail: Option<String>
        }

        impl $error_name {
            $(
                #[allow(non_snake_case)]
                pub fn $error (detail: &str) -> Box<Error> {
                    return Box::new($error_name {
                        kind: $error_kind :: $error,
                        detail: Some(detail.to_owned())
                    });
                }
            )*
        }

        impl fmt::Display for $error_name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match self.detail {
                    Some(ref detail) => write!(f, "{}: {}", self.kind, detail),
                    None => write!(f, "{}", self.kind)
                }
            }
        }

        impl fmt::Display for $error_kind {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match *self {
                    $(
                    $error_kind :: $error => {
                        write!(f, stringify!($error))
                    }
                    ),*
                }
            }
        }

        impl Error for $error_name {
            fn description(&self) -> &str {
                match self.kind {
                    $(
                    $error_kind :: $error => stringify!($error)
                    ),*
                }
            }
        }

        pub type $result_name<T> = Result<T, Box<Error>>;
    };
}

// Produce a "new type" for u64 representing a uid.
macro_rules! make_identifier {
    ($name:ident) => {
        #[derive(Debug, PartialEq, Eq, Hash)]
        pub struct $name(u64);
        impl $name {
            pub fn from_u64(ident: u64) -> $name {
                $name(ident)
            }
            pub fn to_u64(&self) -> u64 {
                let $name(id) = *self;
                return id;
            }
        }
        // FIXME: why can I not just derive Clone?
        impl Clone for $name {
            fn clone(&self) -> $name {
                let $name(ident) = *self;
                return $name(ident);
            }
        }
        // FIXME: ditto Copy
        impl Copy for $name {}
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let $name(ident) = *self;
                write!(f, "{}", ident)
            }
        }
    };
}

