pub mod control {
    #![allow(clippy::all, non_snake_case, non_camel_case_types, unused_imports)]
    include!(concat!(env!("OUT_DIR"), "/control.rs"));
}

pub mod observer {
    #![allow(clippy::all, non_snake_case, non_camel_case_types, unused_imports)]
    include!(concat!(env!("OUT_DIR"), "/observer.rs"));
}

pub mod service;
