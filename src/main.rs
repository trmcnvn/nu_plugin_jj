use nu_plugin::{MsgPackSerializer, serve_plugin};
use nu_plugin_jj::plugin::JjPlugin;

fn main() {
    serve_plugin(&JjPlugin, MsgPackSerializer);
}
