//! Owned component trees. No language is resolved until the game displays a component.
use serde::{Deserialize, Serialize};

/// A component tree, retaining translation keys, styles and ordered children.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[horizon_mod_api_macros::wasm(via = TextForm)]
#[non_exhaustive]
pub struct Text {
    /// The component's contents before its children.
    pub content: TextContent,
    /// Fields not set here inherit from the enclosing component.
    pub style: TextStyle,
    /// Components appended in order.
    pub children: Vec<Text>,
    /// Unrecognised component metadata survives an engine round trip.
    #[serde(default)]
    metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

/// What supplies the characters or inline object of a component.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TextContent {
    /// Literal characters, without interpreting legacy formatting codes.
    Literal(String),
    /// A translation remains a key and components, not a localized string.
    Translatable {
        /// Translation identifier.
        key: String,
        /// Template used when the key is not present in the language.
        fallback: Option<String>,
        /// Ordered `%s` arguments, retaining their own styles.
        arguments: Vec<Text>,
    },
    /// A key binding's localized display name.
    Keybind(String),
    /// A selector resolved by the game's component context.
    Selector {
        /// Selector expression.
        pattern: String,
        /// Component between the selected names.
        separator: Option<Box<Text>>,
    },
    /// A scoreboard value resolved by the game's component context.
    Score {
        /// Score holder or selector.
        name: String,
        /// Scoreboard objective.
        objective: String,
    },
    /// NBT resolved by the game's component context.
    Nbt {
        /// NBT path expression.
        path: String,
        /// Whether the selected strings contain components.
        interpret: bool,
        /// Where to obtain the NBT.
        source: TextNbtSource,
        /// Component between selected values.
        separator: Option<Box<Text>>,
    },
    /// A non-character inline object.
    Object(TextObject),
}

/// The source of an NBT component, not the NBT itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TextNbtSource {
    /// Block position expression.
    Block(String),
    /// Entity selector expression.
    Entity(String),
    /// Command storage identifier.
    Storage(String),
}

/// An inline object embedded in text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TextObject {
    /// A player head, retaining the profile properties carried by the component.
    Player {
        /// Profile data, including signed skin properties when present.
        profile: serde_json::Value,
        /// Whether to draw the hat layer.
        hat: bool,
        /// Optional component for consumers unable to display the object; None uses the default.
        fallback: Option<Box<Text>>,
    },
    /// A sprite in a named atlas.
    Atlas {
        /// Atlas identifier.
        atlas: String,
        /// Sprite identifier.
        sprite: String,
        /// Optional component for consumers unable to display the object; None uses the default.
        fallback: Option<Box<Text>>,
    },
}

/// Optional style fields inherit; explicit `false` overrides an inherited decoration.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TextStyle {
    /// Named colour or `#RRGGBB` component colour.
    pub color: Option<String>,
    /// Packed ARGB shadow colour; zero suppresses shadows.
    pub shadow_color: Option<u32>,
    /// Bold decoration.
    pub bold: Option<bool>,
    /// Italic decoration.
    pub italic: Option<bool>,
    /// Underline decoration.
    pub underlined: Option<bool>,
    /// Strikethrough decoration.
    pub strikethrough: Option<bool>,
    /// Obfuscated decoration.
    pub obfuscated: Option<bool>,
    /// Action invoked by clicking the component.
    pub click: Option<TextClick>,
    /// Information displayed on hover.
    pub hover: Option<TextHover>,
    /// Text inserted into the chat input on Shift-click.
    pub insertion: Option<String>,
    /// Font identifier.
    pub font: Option<String>,
    /// Unknown style metadata remains owned data, without engine types.
    #[serde(default)]
    metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

/// A text component's click action.
///
/// File-opening actions are intentionally absent: mods can neither produce nor
/// round-trip `OpenFile`. Capture drops untrusted file-opening action metadata.
/// Part of the [per-node origin tracking follow-up](https://github.com/arismoko/horizon/issues/1738).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TextClick {
    /// Open an HTTP or HTTPS URL.
    OpenUrl(String),
    /// Switch to the numbered page of a book.
    ChangePage(i32),
    /// Put a command in the chat input.
    SuggestCommand(String),
    /// Run a command.
    RunCommand(String),
    /// Copy text to the clipboard.
    CopyToClipboard(String),
    /// Show a dialog identifier or inline declaration.
    ShowDialog(serde_json::Value),
    /// A server-defined action with optional payload.
    Custom {
        /// Action identifier.
        id: String,
        /// Payload retained without interpretation.
        payload: Option<serde_json::Value>,
    },
}

/// A text component's hover action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TextHover {
    /// Show another styled component tree.
    Text(Box<Text>),
    /// Show an item with its component patch.
    Item {
        /// Item identifier, not a process-local registry slot.
        id: String,
        /// Display count.
        count: u32,
        /// Component patch carried by the component, if any.
        components: Option<serde_json::Value>,
    },
    /// Show an entity's identity and optional name.
    Entity {
        /// Entity type identifier.
        kind: String,
        /// UUID as the high and low halves.
        id: [u64; 2],
        /// Optional styled entity name.
        name: Option<Box<Text>>,
    },
}

impl Default for Text {
    fn default() -> Self {
        Self::literal("")
    }
}
impl Text {
    /// A literal component, with no inherited style overrides or children.
    pub fn literal(value: impl Into<String>) -> Self {
        Self {
            content: TextContent::Literal(value.into()),
            style: TextStyle::default(),
            children: Vec::new(),
            metadata: Default::default(),
        }
    }
    /// A translatable component. The key and arguments remain unresolved data.
    pub fn translate(key: impl Into<String>, arguments: Vec<Text>) -> Self {
        Self {
            content: TextContent::Translatable {
                key: key.into(),
                fallback: None,
                arguments,
            },
            ..Self::default()
        }
    }
    /// Append a child without flattening its style or translation.
    pub fn append(mut self, child: Text) -> Self {
        self.children.push(child);
        self
    }
    /// Set this component's style; children continue to inherit unset fields.
    pub fn with_style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }
}

/// A finite, serializable representation of the recursive tree.
/// Native boundary conversion and generated wasm bindings share the same serde model.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct TextForm {
    /// Serialized owned tree, including component metadata and unresolved keys.
    pub tree: String,
}
/// Maximum nested JSON containers in a text form, below serde_json's recursion limit.
pub const MAX_TEXT_FORM_DEPTH: usize = 100;

/// A text form was too deeply nested or could not be serialized or parsed.
#[derive(Debug)]
#[non_exhaustive]
pub enum TextFormError {
    /// The form exceeds [`MAX_TEXT_FORM_DEPTH`] nested containers.
    TooDeep,
    /// The tree is not a valid serialized text component.
    Invalid(serde_json::Error),
}
impl std::fmt::Display for TextFormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooDeep => write!(f, "text form exceeds maximum depth"),
            Self::Invalid(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for TextFormError {}

// The same streaming scanner protects serialization before serde descends further and
// deserialization before serde's recursion limit. Brackets inside strings do not count.
#[derive(Default)]
struct DepthWriter {
    bytes: Vec<u8>,
    depth: usize,
    quoted: bool,
    escaped: bool,
    too_deep: bool,
}
impl std::io::Write for DepthWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        for &byte in bytes {
            if self.quoted {
                if self.escaped {
                    self.escaped = false;
                } else if byte == b'\\' {
                    self.escaped = true;
                } else if byte == b'"' {
                    self.quoted = false;
                }
            } else {
                match byte {
                    b'"' => self.quoted = true,
                    b'{' | b'[' => {
                        self.depth += 1;
                        if self.depth > MAX_TEXT_FORM_DEPTH {
                            self.too_deep = true;
                            return Err(std::io::Error::other("text form too deep"));
                        }
                    }
                    b'}' | b']' => self.depth = self.depth.saturating_sub(1),
                    _ => {}
                }
            }
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl TryFrom<&Text> for TextForm {
    type Error = TextFormError;
    fn try_from(value: &Text) -> Result<Self, Self::Error> {
        let mut writer = DepthWriter::default();
        if let Err(error) = serde_json::to_writer(&mut writer, value) {
            return Err(if writer.too_deep {
                TextFormError::TooDeep
            } else {
                TextFormError::Invalid(error)
            });
        }
        // serde_json always writes UTF-8.
        Ok(Self {
            tree: String::from_utf8(writer.bytes).expect("JSON is UTF-8"),
        })
    }
}
impl TryFrom<TextForm> for Text {
    type Error = TextFormError;
    fn try_from(value: TextForm) -> Result<Self, Self::Error> {
        use std::io::Write;
        let mut writer = DepthWriter::default();
        writer
            .write_all(value.tree.as_bytes())
            .map_err(|_| TextFormError::TooDeep)?;
        serde_json::from_str(&value.tree).map_err(TextFormError::Invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn form_bounds_depth_and_preserves_hover_and_insertion() {
        let mut text = Text::literal("root").with_style(TextStyle {
            hover: Some(TextHover::Text(Box::new(Text::literal("hover")))),
            insertion: Some("insert".into()),
            ..Default::default()
        });
        let round_trip = Text::try_from(TextForm::try_from(&text).unwrap()).unwrap();
        assert_eq!(round_trip.style.hover, text.style.hover);
        assert_eq!(round_trip.style.insertion, text.style.insertion);
        // Metadata allows exercising the exact JSON-container limit independently of
        // the several containers used to represent each Text node.
        let mut nested = serde_json::json!(0);
        for _ in 0..MAX_TEXT_FORM_DEPTH - 2 {
            nested = serde_json::json!([nested]);
        }
        text.metadata.insert("nested".into(), nested);
        let form = TextForm::try_from(&text).unwrap();
        assert!(Text::try_from(form).is_ok());
        let nested = text.metadata.remove("nested").unwrap();
        text.metadata
            .insert("nested".into(), serde_json::json!([nested]));
        assert!(matches!(
            TextForm::try_from(&text),
            Err(TextFormError::TooDeep)
        ));

        let mut deep = Text::literal("end");
        for _ in 0..100_000 {
            deep = Text::literal("node").append(deep);
        }
        assert!(matches!(
            TextForm::try_from(&deep),
            Err(TextFormError::TooDeep)
        ));
        // Dropping a deeply recursive value is not part of conversion.
        std::mem::forget(deep);
    }
    #[test]
    fn form_refuses_deep_input_and_keeps_all_style_and_source_fields() {
        let raw = format!("{}0{}", "[".repeat(101), "]".repeat(101));
        assert!(matches!(
            Text::try_from(TextForm { tree: raw }),
            Err(TextFormError::TooDeep)
        ));
        let style = TextStyle {
            color: Some("red".into()),
            shadow_color: Some(0xff123456),
            italic: Some(true),
            underlined: Some(true),
            strikethrough: Some(true),
            obfuscated: Some(true),
            click: Some(TextClick::RunCommand("/test".into())),
            font: Some("test:font".into()),
            ..Default::default()
        };
        let mut score = Text::literal("").with_style(style);
        score.content = TextContent::Score {
            name: "test".into(),
            objective: "objective".into(),
        };
        let mut nbt = Text::literal("");
        nbt.content = TextContent::Nbt {
            path: "foo.bar".into(),
            interpret: true,
            source: TextNbtSource::Storage("test:data".into()),
            separator: None,
        };
        let text = score.append(nbt);
        assert_eq!(
            Text::try_from(TextForm::try_from(&text).unwrap()).unwrap(),
            text
        );
    }
    #[test]
    fn via_accepts_infallible_and_fallible_forms() {
        #[horizon_mod_api_macros::wasm(via = String)]
        struct Infallible;
        impl From<&Infallible> for String {
            fn from(_: &Infallible) -> Self {
                String::new()
            }
        }
        impl TryFrom<String> for Infallible {
            type Error = ();
            fn try_from(_: String) -> Result<Self, Self::Error> {
                Ok(Self)
            }
        }
        #[horizon_mod_api_macros::wasm(via = TextForm)]
        struct Fallible(Text);
        impl TryFrom<&Fallible> for TextForm {
            type Error = TextFormError;
            fn try_from(value: &Fallible) -> Result<Self, Self::Error> {
                Self::try_from(&value.0)
            }
        }
        impl TryFrom<TextForm> for Fallible {
            type Error = TextFormError;
            fn try_from(value: TextForm) -> Result<Self, Self::Error> {
                Text::try_from(value).map(Self)
            }
        }
    }
    #[test]
    fn form_retains_translation_children_and_explicit_style_overrides() {
        let text = Text::translate(
            "chat.type.text",
            vec![Text::literal("Alex"), Text::literal("Hi")],
        )
        .with_style(TextStyle {
            bold: Some(true),
            ..Default::default()
        })
        .append(Text::literal("!").with_style(TextStyle {
            bold: Some(false),
            ..Default::default()
        }));
        let decoded = Text::try_from(TextForm::try_from(&text).unwrap()).unwrap();
        assert_eq!(decoded, text);
        assert!(
            matches!(&decoded.content, TextContent::Translatable { key, .. } if key == "chat.type.text")
        );
        assert_eq!(decoded.children[0].style.bold, Some(false));
    }
    #[test]
    fn object_fallback_form_distinguishes_omitted_styled_and_translatable() {
        use serde_json::json;
        let styled = json!({"content":{"Literal":"styled"}, "style":{"bold":false}, "children":[]});
        let translated = json!({"content":{"Translatable":{"key":"test.head", "arguments":[]}}, "style":{}, "children":[]});
        for object in [
            json!({"Player":{"profile":{"name":"Test"}, "hat":true}}),
            json!({"Atlas":{"atlas":"minecraft:blocks", "sprite":"test:icon"}}),
        ] {
            for fallback in [None, Some(styled.clone()), Some(translated.clone())] {
                let mut object = object.clone();
                if let Some(fallback) = &fallback {
                    object.as_object_mut().unwrap().values_mut().next().unwrap()["fallback"] =
                        fallback.clone();
                }
                let tree =
                    json!({"content":{"Object":object}, "style":{}, "children":[]}).to_string();
                let text = Text::try_from(TextForm { tree }).unwrap();
                let TextContent::Object(object) = &text.content else {
                    panic!("object contents")
                };
                let captured = match object {
                    TextObject::Player { fallback, .. } | TextObject::Atlas { fallback, .. } => {
                        fallback
                    }
                };
                match &fallback {
                    None => assert!(captured.is_none()),
                    Some(value) if value == &styled => {
                        assert_eq!(captured.as_ref().unwrap().style.bold, Some(false))
                    }
                    Some(_) => assert!(
                        matches!(&captured.as_ref().unwrap().content, TextContent::Translatable { key, .. } if key == "test.head")
                    ),
                }
                assert_eq!(
                    Text::try_from(TextForm::try_from(&text).unwrap()).unwrap(),
                    text
                );
            }
        }
    }
    #[test]
    fn form_keeps_unknown_metadata_and_refuses_malformed_trees() {
        let mut value = serde_json::to_value(Text::literal("value")).unwrap();
        value["metadata"]["future_component"] = serde_json::json!({"version": 7});
        value["style"]["metadata"]["future_style"] = serde_json::json!([1, 2]);
        let text: Text = serde_json::from_value(value.clone()).unwrap();
        let decoded = Text::try_from(TextForm::try_from(&text).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), value);
        assert!(
            Text::try_from(TextForm {
                tree: "{\"content\": 42}".into()
            })
            .is_err()
        );
    }
}
