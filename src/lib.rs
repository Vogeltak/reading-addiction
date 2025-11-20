pub mod db;
pub mod pocket;
pub mod worker;

pub static USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " bot"
);
