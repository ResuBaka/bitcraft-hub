use anyhow::Context;
use futures::TryStreamExt;
use sea_orm::{DatabaseConnection, EntityTrait, Select};

/// Consume rows as they arrive instead of retaining a full query result vector.
/// Each caller owns its stream/connection for the duration of the load.
pub(crate) async fn stream_into<E: EntityTrait>(
    query: Select<E>,
    conn: &DatabaseConnection,
    mut insert: impl FnMut(E::Model),
) -> anyhow::Result<()> {
    let table = E::default().table_name().to_owned();
    let mut rows = query
        .stream(conn)
        .await
        .with_context(|| format!("Opening startup stream for {table}"))?;
    while let Some(row) = rows
        .try_next()
        .await
        .with_context(|| format!("Reading startup stream for {table}"))?
    {
        insert(row);
    }
    Ok(())
}
