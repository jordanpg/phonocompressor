//! The compiled data file (written by `phono-build`, read by the engine).

use crate::chunks::Translit;
use crate::features::FeatureTable;
use crate::g2p::G2pModel;
use crate::lexicon::Lexicon;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashMap;
use std::hash::BuildHasher;

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

/// `serialize_with` for `HashMap` fields: writes the entries in key order, so the same data
/// always gives the same bytes: a release identifies the file by its checksum.
/// The layout is postcard's for any map (length, then entries), so the format is unchanged and
/// the fields still deserialize as plain `HashMap`s.
pub(crate) fn sorted_map<K, V, H, S>(
    map: &HashMap<K, V, H>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    K: Ord + Serialize,
    V: Serialize,
    H: BuildHasher,
    S: Serializer,
{
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_unstable_by(|a, b| a.0.cmp(b.0));
    serializer.collect_map(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phone::parse_pron;
    use std::collections::BTreeMap;

    /// Enough entries that two `HashMap`s with different random seeds won't happen to iterate
    /// in the same order.
    const N: u32 = 500;

    fn data(ascending: bool) -> Data {
        let mut keys: Vec<u32> = (0..N).collect();
        if !ascending {
            keys.reverse();
        }
        let mut data = Data {
            version: FORMAT_VERSION,
            ..Data::default()
        };
        for &i in &keys {
            let c = char::from_u32(0x4E00 + i).unwrap();
            data.translit.map.insert(c, format!("Chunk{i} "));
            let pron = parse_pron(if i % 2 == 0 { "K AE1 T" } else { "D AO1 G" }).unwrap();
            data.lexicon.words.insert(format!("word{i}"), vec![pron]);
        }
        data
    }

    #[test]
    fn bytes_do_not_depend_on_insertion_order() {
        let (a, b) = (
            data(true).to_bytes().unwrap(),
            data(false).to_bytes().unwrap(),
        );
        assert!(a == b, "same entries serialized to different bytes");
    }

    #[test]
    fn maps_keep_the_postcard_map_layout() {
        // Sorted output must be exactly what postcard writes for any map (length, then
        // entries), so files built before the change still load and FORMAT_VERSION stays.
        let d = data(false);
        let words: BTreeMap<_, _> = d.lexicon.words.iter().collect();
        let translit: BTreeMap<_, _> = d.translit.map.iter().collect();
        let expected =
            postcard::to_allocvec(&(d.version, &d.features, words, &d.g2p, translit)).unwrap();
        assert!(
            d.to_bytes().unwrap() == expected,
            "bytes differ from a sorted postcard map"
        );
    }

    #[test]
    fn sorted_bytes_load_back() {
        let d = data(false);
        let back = Data::from_bytes(&d.to_bytes().unwrap()).unwrap();
        assert_eq!(back.translit.map, d.translit.map);
        assert_eq!(back.lexicon.words, d.lexicon.words);
    }
}
