# nu_plugin_jj

Nushell plugin that reads [Jujutsu](https://github.com/jj-vcs/jj) repository state in-process using `jj-lib`. Provides both structured data and a pre-formatted prompt string.

## Install

```sh
cargo install --path .
plugin add ~/.cargo/bin/nu_plugin_jj
plugin use jj
```

## Commands

### `jj-prompt`

Returns a structured record with raw JJ repo state. Returns `nothing` outside a JJ repo or on error.

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

#### Record fields

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

### `jj-prompt format`

Returns a pre-formatted ANSI-colored string ready for use in a shell prompt. Defaults match [hydro-jj](https://github.com/trmcnvn/hydro-jj) styling.

```nu
> jj-prompt format
󱗆 kxqpzmso main (empty) (no description set)
```

#### Symbols

| Flag | Default | Description |
|---|---|---|
| `--icon` | `󱗆` | Icon symbol |
| `--conflict` | `💥` | Conflict indicator |
| `--divergent` | `🚧` | Divergent indicator |
| `--hidden` | `👻` | Hidden indicator |
| `--immutable` | `🔒` | Immutable indicator |

#### Text

| Flag | Default | Description |
|---|---|---|
| `--empty-text` | `(empty)` | Text for empty commits |
| `--no-desc-text` | `(no description set)` | Text when no description |
| `--change-id-len` | `8` | Change ID display length (non-negative int) |
| `--desc-len` | `29` | Max description length before truncation (non-negative int) |

#### Colors

Colors accept names (`red`, `green`, `blue`, `magenta`, `cyan`, `yellow`, `white`, `black`), modifier prefixes (`bold_`, `dim_`, `italic_`, `bright_`), or hex values (`#rrggbb`). Modifiers and hex can be combined (e.g. `bold_#ebbcba`). `bright_#rrggbb` maps to bold truecolor.

| Flag | Default | Description |
|---|---|---|
| `--icon-color` | `blue` | Icon color |
| `--change-id-color` | `bold_magenta` | Change ID unique prefix color |
| `--change-id-rest-color` | `dim_magenta` | Change ID remainder color |
| `--bookmark-color` | `magenta` | Bookmark name color |
| `--status-color` | `green` | Empty/description text color |

#### Example

```nu
jj-prompt format --icon "⚡" --icon-color cyan --status-color "#9ccfd8" --desc-len 40
```

## Prompt integration

Minimal `prompt.nu` using `jj-prompt format`:

```nu
$env.PROMPT_COMMAND = {||
    let jj = (jj-prompt format)
    let prompt = if ($jj | is-not-empty) { $"($env.PWD) ($jj)" } else { $env.PWD }
    $"($prompt)\n"
}

$env.PROMPT_INDICATOR = {|| "❯ " }
```

## Requirements

- Nushell 0.110
- Rust 1.82+
