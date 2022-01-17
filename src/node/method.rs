//! Method trait and type definitions for attaching associated types to RPC
//! methods.

use serde_json::Value;

/// An Ethereum JSON RPC method.
pub trait Method {
    type Params;
    type Result;

    fn into_name(self) -> String;
}

impl Method for String {
    type Params = Vec<Value>;
    type Result = Value;

    fn into_name(self) -> String {
        self
    }
}

impl Method for &'_ str {
    type Params = <String as Method>::Params;
    type Result = <String as Method>::Result;

    fn into_name(self) -> String {
        self.to_owned()
    }
}

macro_rules! impl_method {
    (
        $(#[$attr:meta])*
        pub struct $m:ident = $s:literal ($($p:ty),*) -> $r:ty;
    ) => {
        $(#[$attr])*
        #[derive(Clone, Copy, Debug, Default)]
        pub struct $m;

        impl $crate::node::method::Method for $m {
            type Params = ($($p,)*);
            type Result = $r;

            fn into_name(self) -> String {
                $s.into_name()
            }
        }

        impl PartialEq<str> for $m {
            fn eq(&self, other: &str) -> bool {
                $s == other
            }
        }
    };
}

pub mod eth {
    use crate::serialization::Bytes;
    use hdwallet::account::Address;

    impl_method! {
        /// Sign a message with a prefix.
        pub struct Sign = "eth_sign" (Address, Bytes<Vec<u8>>) -> Bytes<[u8; 65]>;
    }
}
