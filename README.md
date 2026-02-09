# nu_plugin_jj

Nushell plugin that reads [Jujutsu](https://github.com/jj-vcs/jj) repository state in-process using `jj-lib`. Returns a structured record for use in shell prompts.

## Install

```sh
cargo install --path .
plugin add ~/.cargo/bin/nu_plugin_jj
plugin use jj
```

## Usage

```nu
> jj-prompt
╭───────────────────────┬──────────────────╮
│ repo_root             │ /home/user/repo  │
│ change_id             │ kxqpzmso         │
│ change_id_prefix_len  │ 4                │
│ bookmarks             │ [table 1 row]    │
│ description           │ add feature      │
│ empty                 │ false            │
│ conflict              │ false            │
│ divergent             │ false            │
│ hidden                │ false            │
│ immutable             │ false            │
│ has_remote            │ true             │
│ is_synced             │ true             │
╰───────────────────────┴──────────────────╯
```

Returns `nothing` when not inside a JJ repository or on any error.

## Record fields

| Field | Type | Description |
|---|---|---|
| `repo_root` | string | Workspace root path |
| `change_id` | string | 8-char reverse-hex change ID |
| `change_id_prefix_len` | int | Shortest unique prefix length |
| `bookmarks` | list\<record\> | `[{name: string, distance: int}]` |
| `description` | string | First line of commit description |
| `empty` | bool | Working copy commit is empty |
| `conflict` | bool | Working copy has conflicts |
| `divergent` | bool | Multiple visible commits for same change |
| `hidden` | bool | Commit is hidden |
| `immutable` | bool | Commit is in immutable heads set |
| `has_remote` | bool | Closest bookmark has a remote |
| `is_synced` | bool | Remote target matches local |

## Requirements

- Nushell 0.110
- Rust 1.82+
