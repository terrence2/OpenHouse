// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.

macro_rules! make_error {
    (
        $error_name:ident; { $( $error:ident => $error_type:ident ),* }
    ) => {
        #[derive(Debug)]
        pub enum $error_name {
            $( $error ($error_type) ),*
        }

        impl fmt::Display for $error_name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match *self {
                    $(
                    $error_name :: $error (ref err) => {
                        write!(f, concat!(stringify!($error), ": {}"), err)
                    }
                    ),*
                }
            }
        }

        impl Error for $error_name {
            fn description(&self) -> &str {
                match *self {
                    $(
                    $error_name :: $error (_) => stringify!($error)
                    ),*
                }
            }
        }
    };
}
