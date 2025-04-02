use migration::{DbErr, Migrator, MigratorTrait, SchemaManager, sea_orm::DatabaseConnection};

pub async fn migrate(conn: &DatabaseConnection) -> Result<(), DbErr> {
    let schema_manager = SchemaManager::new(conn);
    Migrator::up(conn, None).await?;
    assert!(schema_manager.has_table("users").await?);
    Ok(())
}
