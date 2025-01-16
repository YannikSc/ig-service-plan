use std::collections::HashMap;
use std::fmt::Formatter;

use serde::de::{DeserializeSeed, EnumAccess, SeqAccess, VariantAccess, Visitor};
use serde::{Deserialize, Deserializer};

#[derive(Clone, Debug, serde::Serialize)]
pub struct ProcessableValue {
    pub is_template: bool,
    pub content: Option<serde_json::Value>,
    pub contents: Vec<Self>,
}

impl ProcessableValue {
    pub fn fixed(content: serde_json::Value) -> Self {
        Self {
            is_template: false,
            content: Some(content),
            contents: Vec::new(),
        }
    }

    pub fn template(content: serde_json::Value) -> Self {
        Self {
            is_template: true,
            content: Some(content),
            contents: Vec::new(),
        }
    }

    pub fn sequence(contents: Vec<Self>) -> Self {
        Self {
            is_template: false,
            content: None,
            contents,
        }
    }

    pub fn render(
        &self,
        variables: &HashMap<String, &dyn strfmt::DisplayStr>,
    ) -> anyhow::Result<serde_json::Value> {
        if let Some(serde_json::Value::String(str)) = &self.content {
            if self.is_template {
                return Ok(serde_json::Value::String(strfmt::strfmt(str, variables)?));
            }
        }

        if !self.contents.is_empty() {
            return Ok(serde_json::Value::Array(
                self.contents
                    .iter()
                    .map(|value| value.render(variables))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            ));
        }

        Ok(self.content.clone().unwrap_or_default())
    }
}

struct ProcessableValueVisitor;

struct TagStringVisitor;

impl Visitor<'_> for TagStringVisitor {
    type Value = String;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a YAML tag string")
    }

    fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_string(string.to_owned())
    }

    fn visit_string<E>(self, string: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if string.is_empty() {
            return Ok("static".to_string());
        }

        Ok(string)
    }
}

impl<'de> DeserializeSeed<'de> for TagStringVisitor {
    type Value = String;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(self)
    }
}

impl<'de> Visitor<'de> for ProcessableValueVisitor {
    type Value = ProcessableValue;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a ProcessableValue")
    }

    fn visit_str<E>(self, data: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_string(data.to_owned())
    }

    fn visit_string<E>(self, data: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ProcessableValue::fixed(serde_json::Value::String(data)))
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ProcessableValue::fixed(serde_json::Value::Bool(v)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ProcessableValue::fixed(serde_json::Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ProcessableValue::fixed(serde_json::Value::Null))
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut output = Vec::new();

        while let Some(value) = seq.next_element_seed(ProcessableValueVisitor)? {
            output.push(value);
        }

        Ok(ProcessableValue::sequence(output))
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        let (tag, contents) = data.variant_seed(TagStringVisitor)?;
        let value = contents.newtype_variant()?;

        let value = match tag.as_str() {
            "static" => ProcessableValue::fixed(value),
            "template" => ProcessableValue::template(value),
            tag => return Err(serde::de::Error::custom(format!("Unknown tag {tag}"))),
        };

        Ok(value)
    }
}

impl<'de> DeserializeSeed<'de> for ProcessableValueVisitor {
    type Value = ProcessableValue;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Deserialize<'de> for ProcessableValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ProcessableValueVisitor)
    }
}
