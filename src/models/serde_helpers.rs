/// Deserialize `Option<i64>` from query strings where integers arrive as strings.
pub fn deserialize_i64_from_string<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    use std::fmt;

    struct I64OrString;

    impl<'de> de::Visitor<'de> for I64OrString {
        type Value = Option<i64>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an integer or a numeric string")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D2: de::Deserializer<'de>>(self, d: D2) -> Result<Self::Value, D2::Error> {
            d.deserialize_any(I64OrString)
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(Some(v))
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(Some(v as i64))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            v.parse::<i64>()
                .map(Some)
                .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(v), &self))
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }

    deserializer.deserialize_option(I64OrString)
}

/// Deserialize `Option<bool>` from query strings where booleans arrive as strings
/// (e.g. `?is_archived=false`).
pub fn deserialize_bool_from_string<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    use std::fmt;

    struct BoolOrString;

    impl<'de> de::Visitor<'de> for BoolOrString {
        type Value = Option<bool>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a boolean or a string \"true\"/\"false\"")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D: de::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            d.deserialize_any(BoolOrString)
        }

        fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
            Ok(Some(v))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            match v {
                "true" => Ok(Some(true)),
                "false" => Ok(Some(false)),
                _ => Err(de::Error::invalid_value(de::Unexpected::Str(v), &self)),
            }
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }

    deserializer.deserialize_option(BoolOrString)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct WrapI64 {
        #[serde(default, deserialize_with = "deserialize_i64_from_string")]
        val: Option<i64>,
    }

    #[derive(Deserialize)]
    struct WrapBool {
        #[serde(default, deserialize_with = "deserialize_bool_from_string")]
        val: Option<bool>,
    }

    // --- deserialize_i64_from_string ---

    #[test]
    fn i64_from_numeric_string() {
        let w: WrapI64 = serde_qs::from_str("val=42").unwrap();
        assert_eq!(w.val, Some(42));
    }

    #[test]
    fn i64_from_negative_string() {
        let w: WrapI64 = serde_qs::from_str("val=-7").unwrap();
        assert_eq!(w.val, Some(-7));
    }

    #[test]
    fn i64_from_non_numeric_string_is_error() {
        let res = serde_qs::from_str::<WrapI64>("val=abc");
        assert!(res.is_err());
    }

    #[test]
    fn i64_absent_is_none() {
        let w: WrapI64 = serde_qs::from_str("").unwrap();
        assert_eq!(w.val, None);
    }

    // --- deserialize_bool_from_string ---

    #[test]
    fn bool_from_true_string() {
        let w: WrapBool = serde_qs::from_str("val=true").unwrap();
        assert_eq!(w.val, Some(true));
    }

    #[test]
    fn bool_from_false_string() {
        let w: WrapBool = serde_qs::from_str("val=false").unwrap();
        assert_eq!(w.val, Some(false));
    }

    #[test]
    fn bool_from_invalid_string_is_error() {
        let res = serde_qs::from_str::<WrapBool>("val=yes");
        assert!(res.is_err());
    }

    #[test]
    fn bool_from_numeric_string_is_error() {
        let res = serde_qs::from_str::<WrapBool>("val=1");
        assert!(res.is_err());
    }

    #[test]
    fn bool_absent_is_none() {
        let w: WrapBool = serde_qs::from_str("").unwrap();
        assert_eq!(w.val, None);
    }
}
