//! The compiled data file (written by `phono-build`, read by the engine).

use crate::chunks::Translit;
use crate::features::FeatureTable;
use crate::g2p::G2pModel;
use crate::lexicon::Lexicon;
use serde::{Deserialize, Serialize};

/// Bump whenever the layout changes; old files are rejected rather than misread.
pub const FORMAT_VERSION: u32 = 2;

#[derive(Serialize, Deserialize, Default)]
pub struct Data {
    pub version: u32,
    pub features: FeatureTable,
    pub lexicon: Lexicon,
    pub g2p: G2pModel,
    pub translit: Translit,
}

impl Data {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        postcard::to_allocvec(self).map_err(|e| e.to_string())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let data: Data =
            postcard::from_bytes(bytes).map_err(|e| format!("corrupt data file: {e}"))?;
        if data.version != FORMAT_VERSION {
            return Err(format!(
                "data file version {} != expected {FORMAT_VERSION}; rebuild it",
                data.version
            ));
        }
        Ok(data)
    }
}
