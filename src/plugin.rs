use std::path::Path;

use nu_plugin::{EngineInterface, EvaluatedCall, Plugin, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Signature, SyntaxShape, Type, Value, record};

use crate::jj;

pub struct JjPlugin;

impl Plugin for JjPlugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn commands(&self) -> Vec<Box<dyn nu_plugin::PluginCommand<Plugin = Self>>> {
        vec![Box::new(JjPromptCommand)]
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
            .optional("path", SyntaxShape::Filepath, "Path to check (defaults to PWD)")
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

        let path_str = match call.opt::<String>(0)? {
            Some(p) => p,
            None => engine
                .get_current_dir()
                .map_err(|e| LabeledError::new(format!("get cwd: {e}")))?,
        };
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
