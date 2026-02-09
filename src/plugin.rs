use std::path::Path;

use nu_plugin::{EngineInterface, EvaluatedCall, Plugin, SimplePluginCommand};
use nu_protocol::{record, Category, LabeledError, Signature, SyntaxShape, Type, Value};

use crate::jj;

pub struct JjPlugin;

impl Plugin for JjPlugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn commands(&self) -> Vec<Box<dyn nu_plugin::PluginCommand<Plugin = Self>>> {
        vec![Box::new(JjPromptCommand), Box::new(JjPromptFormatCommand)]
    }
}

fn resolve_path(engine: &EngineInterface, call: &EvaluatedCall) -> Result<String, LabeledError> {
    match call.opt::<String>(0)? {
        Some(p) => Ok(p),
        None => engine
            .get_current_dir()
            .map_err(|e| LabeledError::new(format!("get cwd: {e}"))),
    }
}

fn parse_non_negative_usize(name: &str, value: i64) -> Result<usize, LabeledError> {
    if value < 0 {
        return Err(LabeledError::new(format!("--{name} must be non-negative")));
    }
    usize::try_from(value).map_err(|_| LabeledError::new(format!("--{name} is too large")))
}

fn color_to_ansi(color: &str) -> String {
    let parts: Vec<&str> = color.splitn(2, '_').collect();

    let (attrs, base) = match parts.as_slice() {
        [attr, base] => {
            let a = match *attr {
                "bold" => "1;",
                "dim" => "2;",
                "italic" => "3;",
                "underline" => "4;",
                "bright" => "9",
                _ => return format!("\x1b[35m"),
            };
            (a, *base)
        }
        [base] => ("", *base),
        _ => return format!("\x1b[35m"),
    };

    if base.starts_with('#') && base.len() == 7 {
        let r = u8::from_str_radix(&base[1..3], 16).unwrap_or(0);
        let g = u8::from_str_radix(&base[3..5], 16).unwrap_or(0);
        let b = u8::from_str_radix(&base[5..7], 16).unwrap_or(0);
        let attr_code = match attrs {
            "1;" => "1;",
            "2;" => "2;",
            "3;" => "3;",
            "4;" => "4;",
            "9" => "1;",
            _ => "",
        };
        return format!("\x1b[{attr_code}38;2;{r};{g};{b}m");
    }

    let fg = if attrs == "9" {
        match base {
            "black" => "90",
            "red" => "91",
            "green" => "92",
            "yellow" => "93",
            "blue" => "94",
            "magenta" => "95",
            "cyan" => "96",
            "white" => "97",
            _ => "95",
        }
    } else {
        match base {
            "black" => "30",
            "red" => "31",
            "green" => "32",
            "yellow" => "33",
            "blue" => "34",
            "magenta" => "35",
            "cyan" => "36",
            "white" => "37",
            _ => "35",
        }
    };

    if attrs == "9" {
        format!("\x1b[{fg}m")
    } else if attrs.is_empty() {
        format!("\x1b[{fg}m")
    } else {
        format!("\x1b[{attrs}{fg}m")
    }
}

struct JjPromptCommand;

impl SimplePluginCommand for JjPromptCommand {
    type Plugin = JjPlugin;

    fn name(&self) -> &str {
        "jj-prompt"
    }

    fn description(&self) -> &str {
        "Get JJ repository status for shell prompt"
    }

    fn signature(&self) -> Signature {
        Signature::build("jj-prompt")
            .optional(
                "path",
                SyntaxShape::Filepath,
                "Path to check (defaults to PWD)",
            )
            .input_output_types(vec![(Type::Nothing, Type::Any)])
            .category(Category::Custom("prompt".into()))
    }

    fn run(
        &self,
        _plugin: &JjPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let span = call.head;
        let path_str = resolve_path(engine, call)?;
        let path = Path::new(&path_str);

        let status = match jj::collect(path) {
            Ok(Some(s)) => s,
            Ok(None) | Err(_) => return Ok(Value::nothing(span)),
        };

        let bookmarks_val: Vec<Value> = status
            .bookmarks
            .iter()
            .map(|b| {
                Value::record(
                    record! {
                        "name" => Value::string(&b.name, span),
                        "distance" => Value::int(b.distance as i64, span),
                    },
                    span,
                )
            })
            .collect();

        Ok(Value::record(
            record! {
                "repo_root" => Value::string(&status.repo_root, span),
                "change_id" => Value::string(&status.change_id, span),
                "change_id_prefix_len" => Value::int(status.change_id_prefix_len as i64, span),
                "bookmarks" => Value::list(bookmarks_val, span),
                "description" => Value::string(&status.description, span),
                "empty" => Value::bool(status.empty, span),
                "conflict" => Value::bool(status.conflict, span),
                "divergent" => Value::bool(status.divergent, span),
                "hidden" => Value::bool(status.hidden, span),
                "immutable" => Value::bool(status.immutable, span),
                "has_remote" => Value::bool(status.has_remote, span),
                "is_synced" => Value::bool(status.is_synced, span),
            },
            span,
        ))
    }
}

struct JjPromptFormatCommand;

const ANSI_RESET: &str = "\x1b[0m";

struct FormatOptions {
    icon: String,
    icon_color: String,
    change_id_color: String,
    change_id_rest_color: String,
    bookmark_color: String,
    status_color: String,
    conflict: String,
    divergent: String,
    hidden: String,
    immutable: String,
    change_id_len: usize,
    empty_text: String,
    no_desc_text: String,
    desc_len: usize,
}

fn format_prompt(status: &jj::JjStatus, options: &FormatOptions) -> String {
    let icon_color = color_to_ansi(&options.icon_color);
    let cid_color = color_to_ansi(&options.change_id_color);
    let cid_rest_color = color_to_ansi(&options.change_id_rest_color);
    let bm_color = color_to_ansi(&options.bookmark_color);
    let status_color = color_to_ansi(&options.status_color);

    let mut parts: Vec<String> = Vec::new();

    parts.push(format!("{icon_color}{}{ANSI_RESET}", options.icon));

    let cid = &status.change_id[..options.change_id_len.min(status.change_id.len())];
    let prefix_len = status.change_id_prefix_len.min(cid.len());
    let cid_prefix = &cid[..prefix_len];
    let cid_rest = &cid[prefix_len..];
    parts.push(format!(
        "{cid_color}{cid_prefix}{ANSI_RESET}{cid_rest_color}{cid_rest}{ANSI_RESET}"
    ));

    let bookmarks: Vec<String> = status
        .bookmarks
        .iter()
        .map(|b| format!("{bm_color}{}{ANSI_RESET}", b.name))
        .collect();
    if !bookmarks.is_empty() {
        parts.push(bookmarks.join(" "));
    }

    let mut flags = String::new();
    if status.conflict {
        flags.push_str(&options.conflict);
    }
    if status.divergent {
        flags.push_str(&options.divergent);
    }
    if status.hidden {
        flags.push_str(&options.hidden);
    }
    if status.immutable {
        flags.push_str(&options.immutable);
    }
    if !flags.is_empty() {
        parts.push(flags);
    }

    if status.empty {
        parts.push(format!("{status_color}{}{ANSI_RESET}", options.empty_text));
    }

    if status.description.is_empty() {
        parts.push(format!(
            "{status_color}{}{ANSI_RESET}",
            options.no_desc_text
        ));
    } else {
        let truncated = if status.description.chars().count() > options.desc_len {
            let s: String = status.description.chars().take(options.desc_len).collect();
            format!("{s}…")
        } else {
            status.description.to_string()
        };
        parts.push(format!("{status_color}{truncated}{ANSI_RESET}"));
    }

    parts.join(" ")
}

impl SimplePluginCommand for JjPromptFormatCommand {
    type Plugin = JjPlugin;

    fn name(&self) -> &str {
        "jj-prompt format"
    }

    fn description(&self) -> &str {
        "Get formatted JJ prompt string with ANSI colors"
    }

    fn signature(&self) -> Signature {
        Signature::build("jj-prompt format")
            .optional(
                "path",
                SyntaxShape::Filepath,
                "Path to check (defaults to PWD)",
            )
            .named("icon", SyntaxShape::String, "Icon symbol", None)
            .named(
                "icon-color",
                SyntaxShape::String,
                "Icon color (default: blue)",
                None,
            )
            .named(
                "change-id-color",
                SyntaxShape::String,
                "Change ID prefix color (default: bold_magenta)",
                None,
            )
            .named(
                "change-id-rest-color",
                SyntaxShape::String,
                "Change ID rest color (default: dim_magenta)",
                None,
            )
            .named(
                "bookmark-color",
                SyntaxShape::String,
                "Bookmark color (default: magenta)",
                None,
            )
            .named(
                "status-color",
                SyntaxShape::String,
                "Empty/description color (default: green)",
                None,
            )
            .named("conflict", SyntaxShape::String, "Conflict symbol", None)
            .named("divergent", SyntaxShape::String, "Divergent symbol", None)
            .named("hidden", SyntaxShape::String, "Hidden symbol", None)
            .named("immutable", SyntaxShape::String, "Immutable symbol", None)
            .named("change-id-len", SyntaxShape::Int, "Change ID length", None)
            .named(
                "empty-text",
                SyntaxShape::String,
                "Text for empty commits",
                None,
            )
            .named(
                "no-desc-text",
                SyntaxShape::String,
                "Text when no description",
                None,
            )
            .named(
                "desc-len",
                SyntaxShape::Int,
                "Max description length before truncation",
                None,
            )
            .input_output_types(vec![(Type::Nothing, Type::Any)])
            .category(Category::Custom("prompt".into()))
    }

    fn run(
        &self,
        _plugin: &JjPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let span = call.head;
        let path_str = resolve_path(engine, call)?;
        let path = Path::new(&path_str);

        let status = match jj::collect(path) {
            Ok(Some(s)) => s,
            Ok(None) | Err(_) => return Ok(Value::nothing(span)),
        };

        let options = FormatOptions {
            icon: call
                .get_flag::<String>("icon")?
                .unwrap_or_else(|| "󱗆".to_string()),
            icon_color: call
                .get_flag::<String>("icon-color")?
                .unwrap_or_else(|| "blue".to_string()),
            change_id_color: call
                .get_flag::<String>("change-id-color")?
                .unwrap_or_else(|| "bold_magenta".to_string()),
            change_id_rest_color: call
                .get_flag::<String>("change-id-rest-color")?
                .unwrap_or_else(|| "dim_magenta".to_string()),
            bookmark_color: call
                .get_flag::<String>("bookmark-color")?
                .unwrap_or_else(|| "magenta".to_string()),
            status_color: call
                .get_flag::<String>("status-color")?
                .unwrap_or_else(|| "green".to_string()),
            conflict: call
                .get_flag::<String>("conflict")?
                .unwrap_or_else(|| "💥".to_string()),
            divergent: call
                .get_flag::<String>("divergent")?
                .unwrap_or_else(|| "🚧".to_string()),
            hidden: call
                .get_flag::<String>("hidden")?
                .unwrap_or_else(|| "👻".to_string()),
            immutable: call
                .get_flag::<String>("immutable")?
                .unwrap_or_else(|| "🔒".to_string()),
            change_id_len: parse_non_negative_usize(
                "change-id-len",
                call.get_flag::<i64>("change-id-len")?.unwrap_or(8),
            )?,
            empty_text: call
                .get_flag::<String>("empty-text")?
                .unwrap_or_else(|| "(empty)".to_string()),
            no_desc_text: call
                .get_flag::<String>("no-desc-text")?
                .unwrap_or_else(|| "(no description set)".to_string()),
            desc_len: parse_non_negative_usize(
                "desc-len",
                call.get_flag::<i64>("desc-len")?.unwrap_or(29),
            )?,
        };

        Ok(Value::string(format_prompt(&status, &options), span))
    }
}

#[cfg(test)]
mod tests {
    use super::{color_to_ansi, format_prompt, parse_non_negative_usize, FormatOptions};
    use crate::jj::{Bookmark, JjStatus};

    fn strip_ansi(input: &str) -> String {
        let mut out = String::new();
        let mut chars = input.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\u{1b}' && chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c == 'm' {
                        break;
                    }
                }
            } else {
                out.push(ch);
            }
        }
        out
    }

    fn test_status(description: &str) -> JjStatus {
        JjStatus {
            repo_root: "/tmp/repo".to_string(),
            change_id: "abcdefgh".to_string(),
            change_id_prefix_len: 4,
            bookmarks: vec![Bookmark {
                name: "main".to_string(),
                distance: 0,
            }],
            description: description.to_string(),
            empty: true,
            conflict: true,
            divergent: false,
            hidden: true,
            immutable: false,
            has_remote: true,
            is_synced: true,
        }
    }

    fn test_options(desc_len: usize) -> FormatOptions {
        FormatOptions {
            icon: "*".to_string(),
            icon_color: "blue".to_string(),
            change_id_color: "bold_magenta".to_string(),
            change_id_rest_color: "dim_magenta".to_string(),
            bookmark_color: "magenta".to_string(),
            status_color: "green".to_string(),
            conflict: "C".to_string(),
            divergent: "D".to_string(),
            hidden: "H".to_string(),
            immutable: "I".to_string(),
            change_id_len: 8,
            empty_text: "(empty)".to_string(),
            no_desc_text: "(no description set)".to_string(),
            desc_len,
        }
    }

    #[test]
    fn rejects_negative_lengths() {
        let err = parse_non_negative_usize("desc-len", -1).unwrap_err();
        assert!(err.to_string().contains("non-negative"));
    }

    #[test]
    fn parses_non_negative_lengths() {
        let value = parse_non_negative_usize("change-id-len", 12).unwrap();
        assert_eq!(value, 12);
    }

    #[test]
    fn bright_hex_applies_modifier() {
        assert_eq!(color_to_ansi("bright_#112233"), "\x1b[1;38;2;17;34;51m");
    }

    #[test]
    fn bright_named_color_uses_bright_ansi() {
        assert_eq!(color_to_ansi("bright_red"), "\x1b[91m");
    }

    #[test]
    fn allows_zero_lengths() {
        let value = parse_non_negative_usize("desc-len", 0).unwrap();
        assert_eq!(value, 0);
    }

    #[test]
    fn invalid_color_falls_back_to_magenta() {
        assert_eq!(color_to_ansi("unknown"), "\x1b[35m");
        assert_eq!(color_to_ansi("bad_red"), "\x1b[35m");
    }

    #[test]
    fn format_output_order_is_stable() {
        let rendered = format_prompt(&test_status("desc"), &test_options(29));
        let plain = strip_ansi(&rendered);
        assert_eq!(plain, "* abcdefgh main CH (empty) desc");
    }

    #[test]
    fn desc_len_boundaries_work() {
        let over = strip_ansi(&format_prompt(&test_status("hello"), &test_options(4)));
        assert!(over.ends_with("hell…"));

        let exact = strip_ansi(&format_prompt(&test_status("hello"), &test_options(5)));
        assert!(exact.ends_with("hello"));

        let zero = strip_ansi(&format_prompt(&test_status("hello"), &test_options(0)));
        assert!(zero.ends_with("…"));
    }
}
