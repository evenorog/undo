#![allow(dead_code)]

use crate::Entry;
use arrayvec::ArrayVec;
use serde::{Deserialize, Serialize};

#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(bound(serialize = "C: Serialize", deserialize = "C: Deserialize<'de>"))
)]
#[derive(Clone)]
pub struct Timeline<C> {
    entries: ArrayVec<[Entry<C>; 32]>,
    current: usize,
}

impl<C> Timeline<C> {
    pub fn new() -> Timeline<C> {
        Timeline {
            entries: ArrayVec::new(),
            current: 0,
        }
    }
}
