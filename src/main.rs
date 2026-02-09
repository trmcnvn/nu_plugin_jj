use nu_plugin::{serve_plugin, MsgPackSerializer};
use nu_plugin_jj::plugin::JjPlugin;

fn main() {
    serve_plugin(&JjPlugin, MsgPackSerializer);
}
