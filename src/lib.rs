//! Library that implements the NBP packet protocol - http://lea.hamradio.si/~s53mv/nbp/nbp.html
extern crate byteorder;
#[macro_use]
extern crate log;
extern crate fern;
extern crate time;
extern crate rand;

pub mod kiss;
pub mod nbp;
pub mod util;