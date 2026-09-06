//! Parse all JSON values while rejecting duplicate object keys at every depth.
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::{collections::BTreeSet, fmt};

struct Checked;

impl<'de> Deserialize<'de> for Checked {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(CheckVisitor)
    }
}

struct CheckVisitor;

impl<'de> Visitor<'de> for CheckVisitor {
    type Value = Checked;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate keys")
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Checked, E> {
        Ok(Checked)
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Checked, E> {
        Ok(Checked)
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Checked, E> {
        Ok(Checked)
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Checked, E> {
        Ok(Checked)
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<Checked, E> {
        Ok(Checked)
    }
    fn visit_none<E: de::Error>(self) -> Result<Checked, E> {
        Ok(Checked)
    }
    fn visit_unit<E: de::Error>(self) -> Result<Checked, E> {
        Ok(Checked)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Checked, A::Error> {
        while sequence.next_element::<Checked>()?.is_some() {}
        Ok(Checked)
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Checked, A::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom(format!("duplicate object key {key}")));
            }
            map.next_value::<Checked>()?;
        }
        Ok(Checked)
    }
}

pub(crate) fn validate(text: &str) -> Result<(), serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    Checked::deserialize(&mut deserializer)?;
    deserializer.end()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_keys_cannot_hide_contract_fields() {
        assert!(validate(r#"{"fields":[{"kind":"first","kind":"second"}]}"#).is_err());
        assert!(validate(r#"{"kind":0,"\u006bind":1}"#).is_err());
        assert!(validate(r#"{"one":{"kind":0},"two":{"kind":1},"null":null}"#).is_ok());
        assert!(validate("{} {}").is_err());
    }
}
