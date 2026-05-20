use std::fmt::Debug;
use std::hash::Hash;

use serde::{Deserialize, Serialize};

trait Sealed {}
pub trait StatusMarker: Sealed + Debug + Eq + PartialEq + Copy + Clone + Hash + Serialize {}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash, Serialize, Deserialize)]
pub struct Serialized;
#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash, Serialize)]
pub struct Active;

impl Sealed for Serialized {}
impl StatusMarker for Serialized {}
impl Sealed for Active {}
impl StatusMarker for Active {}
