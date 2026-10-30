//! Root indexes and recursive directory normalization with immediate-child views.
use crate::collections::nested_array;
use crate::lists::check;
use crate::read_support::{Budget, validate_identity};
use crate::song::{array, id, number, object, required_text, text};
use crate::song_order::{Row, sorted};
use crate::{Auth, ExchangeError, PostPreference, ProtocolTransport, ServerBase, plan_request};
use feishin_domain::*;
use serde_json::{Map, Value};
type Result<T> = std::result::Result<T, ExchangeError>;
struct Node {
    folder: Folder,
    folders: Vec<Folder>,
    songs: Vec<Row>,
}
impl Node {
    fn finish(mut self) -> Folder {
        self.folder.children = Some(FolderChildren {
            folders: self.folders,
            songs: self.songs.into_iter().map(|r| r.song).collect(),
        });
        self.folder
    }
}
fn image(obj: &Map<String, Value>) -> Result<Field<String>> {
    Ok(id(obj, "coverArt")?
        .filter(|id| !id.is_empty())
        .map(Field::Value)
        .unwrap_or(Field::Null))
}
fn node(
    obj: &Map<String, Value>,
    server: &ServerId,
    context: OperationContext<'_>,
    budget: &mut Budget,
) -> Result<Node> {
    budget.take(1)?;
    let mut folders = Vec::new();
    let mut songs = Vec::new();
    for value in array(obj, "child", context)?.unwrap_or(&[]) {
        let child = object(value)?;
        if child.get("isDir") == Some(&Value::Bool(true)) {
            folders.push(node(child, server, context, budget)?.finish());
        } else {
            budget.take(1)?;
            budget.metadata(
                child,
                &["artists", "albumArtists", "contributors", "genres"],
                context,
            )?;
            let song = crate::song::normalize(child, server, context)?;
            songs.push(Row {
                song,
                duration_ms: number(child, "duration")?.unwrap_or(0.0) * 1000.0,
            });
        }
    }
    Ok(Node {
        folder: Folder {
            identity: Scoped {
                server: server.clone(),
                id: FolderId::new(id(obj, "id")?.ok_or(ExchangeError::Malformed)?),
            },
            server_type: ServerType::Subsonic,
            name: required_text(obj, "title")?,
            parent_id: text(obj, "parent")?.map(FolderId::new),
            image_id: image(obj)?,
            image_url: Field::Null,
            children: None,
        },
        folders,
        songs,
    })
}
fn order(folders: &mut [Folder], direction: SortOrder) {
    // Cache keys once; do not lowercase entire names on every comparison.
    if direction == SortOrder::Desc {
        folders.sort_by_cached_key(|f| {
            std::cmp::Reverse(f.name.to_lowercase().encode_utf16().collect::<Vec<_>>())
        });
    } else {
        folders.sort_by_cached_key(|f| f.name.to_lowercase().encode_utf16().collect::<Vec<_>>());
    }
}
/// Caller binds scoped query/base/auth. Root '0' is synthetic; no directory
/// request or local song shuffle occurs there. Only immediate children are sorted.
pub async fn get_folder(
    transport: &dyn ProtocolTransport,
    clock: &dyn Clock,
    context: OperationContext<'_>,
    base: &ServerBase,
    auth: &Auth,
    query: &FolderQuery,
    random: &mut dyn Random,
) -> Result<Folder> {
    check(
        clock,
        context,
        &query.identity.server,
        0,
        None,
        query.search_term.as_deref(),
    )?;
    validate_identity(&query.identity.server, query.identity.id.as_str())?;
    if let Some(folders) = &query.music_folder_ids {
        context
            .check_items(folders.len())
            .map_err(ExchangeError::Context)?;
    }
    let root = query.identity.id.as_str() == "0";
    let pairs = if root {
        query
            .music_folder_ids
            .as_ref()
            .and_then(|v| v.first())
            .map(|id| vec![("musicFolderId", id.as_str())])
            .unwrap_or_default()
    } else {
        vec![("id", query.identity.id.as_str())]
    };
    let response = crate::protocol::exchange(
        transport,
        clock,
        context,
        plan_request(
            base,
            auth,
            if root {
                "getIndexes.view"
            } else {
                "getMusicDirectory.view"
            },
            &pairs,
            PostPreference::Get,
            None,
        )?,
    )
    .await?;
    let result = (|| {
        let term = query
            .search_term
            .as_deref()
            .filter(|v| !v.is_empty())
            .map(str::to_lowercase);
        let mut budget = Budget {
            limit: context.limits.items,
            used: 0,
        };
        if root {
            budget.take(1)?;
            let mut folders = Vec::new();
            for index in nested_array(&response, "indexes", "index", context)? {
                budget.take(1)?;
                for row in
                    array(object(index)?, "artist", context)?.ok_or(ExchangeError::Malformed)?
                {
                    budget.take(1)?;
                    let obj = object(row)?;
                    // Source creates raw ID/title projections before filtering.
                    let id = FolderId::new(id(obj, "id")?.ok_or(ExchangeError::Malformed)?);
                    let name = required_text(obj, "name")?;
                    if term
                        .as_ref()
                        .is_some_and(|t| !name.to_lowercase().contains(t))
                    {
                        continue;
                    }
                    folders.push(Folder {
                        identity: Scoped {
                            server: query.identity.server.clone(),
                            id,
                        },
                        server_type: ServerType::Subsonic,
                        name,
                        parent_id: None,
                        image_id: image(obj)?,
                        image_url: Field::Null,
                        children: Some(FolderChildren {
                            folders: vec![],
                            songs: vec![],
                        }),
                    });
                }
            }
            order(&mut folders, query.sort_order);
            return Ok(Folder {
                identity: query.identity.clone(),
                server_type: ServerType::Subsonic,
                name: "~".into(),
                parent_id: None,
                image_id: Field::Absent,
                image_url: Field::Absent,
                children: Some(FolderChildren {
                    folders,
                    songs: vec![],
                }),
            });
        }
        let mut result = node(
            object(response.get("directory").ok_or(ExchangeError::Malformed)?)?,
            &query.identity.server,
            context,
            &mut budget,
        )?;
        // Continue the detail-read stale-identity policy from ADR-016.
        if result.folder.identity != query.identity {
            return Err(ExchangeError::Stale);
        }
        if let Some(term) = term {
            result
                .folders
                .retain(|f| f.name.to_lowercase().contains(&term));
            result.songs.retain(|r| {
                r.song.name.to_lowercase().contains(&term)
                    || r.song
                        .album
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&term)
                    || r.song.artist_name.to_lowercase().contains(&term)
            });
        }
        order(&mut result.folders, query.sort_order);
        result.songs = sorted(result.songs, query.sort_by, query.sort_order, random)?;
        Ok(result.finish())
    })();
    context.check(clock.now()).map_err(ExchangeError::Context)?;
    result
}
