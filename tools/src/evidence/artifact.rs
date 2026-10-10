//! A bounded, typed retrieval pin. Structural validity does not authenticate it.
use super::identity;
use crate::Result;
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::fmt;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields {
    schema: String,
    repository: String,
    repository_id: u64,
    run_id: u64,
    run_attempt: u64,
    source_commit: String,
    artifact_id: u64,
    artifact_name: String,
    archive_sha256: String,
    archive_bytes: u64,
    member: String,
    expires_at_unix: u64,
}

pub(super) struct Locator(Fields);

impl<'de> Deserialize<'de> for Locator {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct Object;
        impl<'de> Visitor<'de> for Object {
            type Value = Locator;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an artifact locator object")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                map: A,
            ) -> std::result::Result<Locator, A::Error> {
                let locator = Locator(Fields::deserialize(MapAccessDeserializer::new(map))?);
                locator.validate().map_err(serde::de::Error::custom)?;
                Ok(locator)
            }
        }
        deserializer.deserialize_map(Object)
    }
}

pub(super) fn optional<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Locator>, D::Error> {
    // Missing is permitted by serde(default); an explicitly supplied null is not.
    Locator::deserialize(deserializer).map(Some)
}

impl Locator {
    pub(super) fn validate(&self) -> Result<()> {
        let value = &self.0;
        if value.schema != "nepl3.github-artifact/1" || value.repository != "neknaj/NEPL3" {
            return Err("unsupported artifact schema or repository".into());
        }
        if [
            value.repository_id,
            value.run_id,
            value.run_attempt,
            value.artifact_id,
        ]
        .iter()
        .any(|id| !(1..(1 << 53)).contains(id))
        {
            return Err("artifact identifiers must be positive exact JSON integers".into());
        }
        if value.source_commit.len() != 40
            || !value
                .source_commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("invalid artifact source commit".into());
        }
        if !identity::is_digest(&value.archive_sha256)
            || !(22..=32 * 1024 * 1024).contains(&value.archive_bytes)
        {
            return Err("invalid artifact archive size or digest".into());
        }
        if value.artifact_name.is_empty()
            || value.artifact_name.len() > 128
            || !value
                .artifact_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err("invalid artifact name".into());
        }
        let member = &value.member;
        if member.is_empty()
            || member.len() > 96
            || !member.as_bytes()[0].is_ascii_alphanumeric()
            || !member.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
            })
            || member.ends_with('.')
            || member.contains("..")
            || ![".stdout", ".stderr", ".txt", ".log"]
                .iter()
                .any(|suffix| member.ends_with(suffix))
        {
            return Err("invalid artifact member name".into());
        }
        let stem = member
            .split('.')
            .next()
            .ok_or("missing artifact member stem")?;
        if matches!(stem, "con" | "prn" | "aux" | "nul")
            || (stem.len() == 4
                && (stem.starts_with("com") || stem.starts_with("lpt"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            return Err("reserved artifact member name".into());
        }
        if !(1..=253_402_300_799).contains(&value.expires_at_unix) {
            return Err("invalid artifact expiry timestamp".into());
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::{Value, json};

    pub(in crate::evidence) fn fixture() -> Value {
        // Synthetic transport metadata, never an actual artifact or acceptance.
        json!({"schema":"nepl3.github-artifact/1","repository":"neknaj/NEPL3","repository_id":1,
            "run_id":2,"run_attempt":1,"source_commit":"a".repeat(40),"artifact_id":3,
            "artifact_name":"synthetic-test","archive_sha256":"b".repeat(64),"archive_bytes":100,
            "member":"run.stdout","expires_at_unix":1})
    }

    #[test]
    fn optional_pin_preserves_command_and_review_variants() -> Result<()> {
        for mut run in [
            json!({"kind":"command","command":"synthetic fixture","target":"native","result":"passed",
                "exit_code":0,"checks":["synthetic"],"environment":{"runner":{"name":"fixture","version":"1"},
                "tools":[{"name":"fixture","version":"1"}]},"log":"dist/evidence/fixture.log","log_sha256":"a".repeat(64)}),
            json!({"kind":"review","target":"synthetic-review","reviewer":"fixture reviewer","independent":true,
                "decision":"rejected","scope":["synthetic review only"],"log":"dist/evidence/review.log","log_sha256":"b".repeat(64)}),
        ] {
            let _: super::super::Run = serde_json::from_value(run.clone())?;
            run["artifact"] = fixture();
            let _: super::super::Run = serde_json::from_value(run.clone())?;
            let encoded = serde_json::to_string(&run)?;
            let doubled = encoded.replacen('{', &format!("{{\"artifact\":{},", fixture()), 1);
            assert!(serde_json::from_str::<super::super::Run>(&doubled).is_err());
            for invalid in [json!(null), json!([]), json!("pin")] {
                run["artifact"] = invalid;
                assert!(serde_json::from_value::<super::super::Run>(run.clone()).is_err());
            }
        }
        Ok(())
    }

    #[test]
    fn accepts_only_bounded_object_pins() -> Result<()> {
        let good = fixture();
        let _: Locator = serde_json::from_value(good.clone())?;
        for (key, invalid) in [
            ("schema", json!("unknown")),
            ("repository", json!("other/repo")),
            ("repository_id", json!(0)),
            ("run_id", json!(1u64 << 53)),
            ("run_attempt", json!(-1)),
            ("artifact_id", json!(true)),
            ("source_commit", json!("A".repeat(40))),
            ("archive_sha256", json!("B".repeat(64))),
            ("archive_bytes", json!(33 * 1024 * 1024)),
            ("artifact_name", json!("../bad/name")),
            ("member", json!("../run.stdout")),
            ("member", json!("con.stdout")),
            ("member", json!("lpt1.stdout")),
            ("expires_at_unix", json!(253_402_300_800u64)),
            ("source_commit", json!(format!("{}\n", "a".repeat(40)))),
            ("archive_sha256", json!(format!("{}\n", "b".repeat(64)))),
            ("artifact_name", json!("synthetic-test\n")),
            ("member", json!("run.stdout\n")),
            ("extra", json!(1)),
        ] {
            let mut bad = good.clone();
            bad[key] = invalid;
            assert!(serde_json::from_value::<Locator>(bad).is_err(), "{key}");
        }
        for bad in [json!(null), json!([]), json!(false), json!("locator")] {
            assert!(serde_json::from_value::<Locator>(bad).is_err());
        }
        let integer_json = serde_json::to_string(&good)?;
        for token in ["2.0", "2e0"] {
            let raw = integer_json.replace("\"run_id\":2", &format!("\"run_id\":{token}"));
            assert!(serde_json::from_str::<Locator>(&raw).is_err());
        }
        let fields = good.as_object().ok_or("fixture must be object")?;
        for key in fields.keys() {
            let mut bad = good.clone();
            bad.as_object_mut()
                .ok_or("fixture must be object")?
                .remove(key);
            assert!(
                serde_json::from_value::<Locator>(bad).is_err(),
                "missing {key}"
            );
        }
        let positional = json!([
            good["schema"],
            good["repository"],
            good["repository_id"],
            good["run_id"],
            good["run_attempt"],
            good["source_commit"],
            good["artifact_id"],
            good["artifact_name"],
            good["archive_sha256"],
            good["archive_bytes"],
            good["member"],
            good["expires_at_unix"]
        ]);
        assert!(serde_json::from_value::<Locator>(positional).is_err());
        for (key, accepted, rejected) in [
            (
                "run_id",
                json!(9007199254740991u64),
                json!(9007199254740992u64),
            ),
            ("archive_bytes", json!(22), json!(21)),
            ("archive_bytes", json!(33554432), json!(33554433)),
            (
                "expires_at_unix",
                json!(253402300799u64),
                json!(253402300800u64),
            ),
            (
                "artifact_name",
                json!("a".repeat(128)),
                json!("a".repeat(129)),
            ),
            (
                "member",
                json!(format!("{}.log", "a".repeat(92))),
                json!(format!("{}.log", "a".repeat(93))),
            ),
        ] {
            let mut boundary = good.clone();
            boundary[key] = accepted;
            let _: Locator = serde_json::from_value(boundary.clone())?;
            boundary[key] = rejected;
            assert!(
                serde_json::from_value::<Locator>(boundary).is_err(),
                "{key} endpoint"
            );
        }
        let raw = serde_json::to_string(&good)?.replacen('{', "{\"run_id\":2,", 1);
        assert!(serde_json::from_str::<Locator>(&raw).is_err());
        Ok(())
    }
}
