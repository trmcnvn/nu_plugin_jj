use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use jj_lib::backend::CommitId;
use jj_lib::config::{ConfigLayer, ConfigSource, StackedConfig};
use jj_lib::hex_util::encode_reverse_hex;
use jj_lib::object_id::ObjectId;
use jj_lib::repo::Repo;
use jj_lib::repo::StoreFactories;
use jj_lib::settings::UserSettings;
use jj_lib::str_util::StringMatcher;
use jj_lib::workspace::{Workspace, default_working_copy_factories};

use crate::error::Error;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Bookmark {
    pub name: String,
    pub distance: usize,
}

#[derive(Debug)]
pub struct JjStatus {
    pub repo_root: String,
    pub change_id: String,
    pub change_id_prefix_len: usize,
    pub bookmarks: Vec<Bookmark>,
    pub description: String,
    pub empty: bool,
    pub conflict: bool,
    pub divergent: bool,
    pub hidden: bool,
    pub immutable: bool,
    pub has_remote: bool,
    pub is_synced: bool,
}

pub fn collect(path: &Path) -> Result<Option<JjStatus>> {
    let repo_root = match find_repo_root(path) {
        Some(root) => root,
        None => return Ok(None),
    };

    let settings = create_user_settings()?;

    let workspace = Workspace::load(
        &settings,
        &repo_root,
        &StoreFactories::default(),
        &default_working_copy_factories(),
    )
    .map_err(|e| Error::Jj(format!("load workspace: {e}")))?;

    let repo = workspace
        .repo_loader()
        .load_at_head()
        .map_err(|e| Error::Jj(format!("load repo: {e}")))?;

    let view = repo.view();

    let wc_id = match view.wc_commit_ids().get(workspace.workspace_name()) {
        Some(id) => id.clone(),
        None => return Ok(None),
    };

    let commit = repo
        .store()
        .get_commit(&wc_id)
        .map_err(|e| Error::Jj(format!("get commit: {e}")))?;

    let change_id_full = encode_reverse_hex(commit.change_id().as_bytes());
    let change_id_prefix_len = repo
        .shortest_unique_change_id_prefix_len(commit.change_id())
        .unwrap_or(8)
        .min(change_id_full.len());
    let change_id = change_id_full[..8.min(change_id_full.len())].to_string();

    let empty = commit
        .is_empty(repo.as_ref())
        .map_err(|e| Error::Jj(format!("check empty: {e}")))?;

    let conflict = commit.has_conflict();

    let divergent = repo
        .resolve_change_id(commit.change_id())
        .ok()
        .flatten()
        .is_some_and(|resolved| resolved.visible_with_offsets().count() > 1);

    let hidden = commit.is_hidden(repo.as_ref()).unwrap_or(false);

    let immutable_heads = find_immutable_heads(view);
    let immutable = immutable_heads.contains(&wc_id);

    let description = commit
        .description()
        .lines()
        .next()
        .unwrap_or("")
        .to_string();

    let mut bookmarks: Vec<Bookmark> = view
        .local_bookmarks_for_commit(&wc_id)
        .map(|(name, _)| Bookmark {
            name: name.as_str().to_string(),
            distance: 0,
        })
        .collect();

    let ancestor_bookmarks =
        find_ancestor_bookmarks(&repo, view, &wc_id, &immutable_heads, 10)?;
    bookmarks.extend(ancestor_bookmarks);

    let (has_remote, is_synced) = check_remote_sync(view, &bookmarks);

    Ok(Some(JjStatus {
        repo_root: repo_root.to_string_lossy().to_string(),
        change_id,
        change_id_prefix_len,
        bookmarks,
        description,
        empty,
        conflict,
        divergent,
        hidden,
        immutable,
        has_remote,
        is_synced,
    }))
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".jj").is_dir() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn create_user_settings() -> Result<UserSettings> {
    let mut config = StackedConfig::with_defaults();
    let mut layer = ConfigLayer::empty(ConfigSource::User);
    layer
        .set_value("user.name", "nu_plugin_jj")
        .map_err(|e| Error::Jj(format!("set user.name: {e}")))?;
    layer
        .set_value("user.email", "nu_plugin_jj@localhost")
        .map_err(|e| Error::Jj(format!("set user.email: {e}")))?;
    config.add_layer(layer);
    UserSettings::from_config(config).map_err(|e| Error::Jj(format!("settings: {e}")))
}

fn find_immutable_heads(view: &jj_lib::view::View) -> HashSet<CommitId> {
    let mut immutable = HashSet::new();

    for (symbol, remote_ref) in
        view.remote_bookmarks_matching(&StringMatcher::All, &StringMatcher::All)
    {
        let name = symbol.name.as_str();
        let remote = symbol.remote.as_str();

        if remote == "git" {
            continue;
        }

        let is_trunk =
            matches!(remote, "origin" | "upstream") && matches!(name, "main" | "master" | "trunk");
        let is_untracked = view.get_local_bookmark(symbol.name).is_absent();

        if is_trunk || is_untracked {
            if let Some(id) = remote_ref.target.as_normal() {
                immutable.insert(id.clone());
            }
        }
    }

    for (_, target) in view.tags() {
        if let Some(id) = target.local_target.as_normal() {
            immutable.insert(id.clone());
        }
    }

    immutable
}

fn find_ancestor_bookmarks(
    repo: &std::sync::Arc<jj_lib::repo::ReadonlyRepo>,
    view: &jj_lib::view::View,
    wc_id: &CommitId,
    immutable_heads: &HashSet<CommitId>,
    max_depth: usize,
) -> Result<Vec<Bookmark>> {
    let mut queue: VecDeque<(CommitId, usize)> = VecDeque::new();
    let mut visited = HashSet::new();
    let mut found: HashMap<String, usize> = HashMap::new();

    let wc_commit = repo
        .store()
        .get_commit(wc_id)
        .map_err(|e| Error::Jj(format!("get commit: {e}")))?;

    for parent_id in wc_commit.parent_ids() {
        queue.push_back((parent_id.clone(), 1));
    }

    while let Some((commit_id, depth)) = queue.pop_front() {
        if depth > max_depth {
            continue;
        }
        if !visited.insert(commit_id.clone()) {
            continue;
        }

        for (bookmark_name, _) in view.local_bookmarks_for_commit(&commit_id) {
            let name = bookmark_name.as_str().to_string();
            found.entry(name).or_insert(depth);
        }

        if immutable_heads.contains(&commit_id) {
            continue;
        }

        if depth < max_depth {
            let commit = repo
                .store()
                .get_commit(&commit_id)
                .map_err(|e| Error::Jj(format!("get commit: {e}")))?;
            for parent_id in commit.parent_ids() {
                queue.push_back((parent_id.clone(), depth + 1));
            }
        }
    }

    let mut result: Vec<Bookmark> = found
        .into_iter()
        .map(|(name, distance)| Bookmark { name, distance })
        .collect();
    result.sort_by_key(|b| b.distance);
    Ok(result)
}

fn check_remote_sync(view: &jj_lib::view::View, bookmarks: &[Bookmark]) -> (bool, bool) {
    if bookmarks.is_empty() {
        return (false, true);
    }

    let bm_name = &bookmarks[0].name;
    let local_target = view.get_local_bookmark(&jj_lib::ref_name::RefName::new(bm_name));

    let name_matcher = jj_lib::str_util::StringPattern::exact(bm_name).to_matcher();
    let mut has_remote = false;
    let mut is_synced = false;

    for (symbol, remote_ref) in view.remote_bookmarks_matching(&name_matcher, &StringMatcher::All)
    {
        if symbol.remote.as_str() == "git" {
            continue;
        }
        has_remote = true;
        if remote_ref.target == *local_target {
            is_synced = true;
            break;
        }
    }

    (has_remote, is_synced || !has_remote)
}
