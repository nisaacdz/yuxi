use models::domains::{otp, sea_orm_active_enums, texts, tournaments, users};
use sea_orm_migration::prelude::{extension::postgres::Type, *};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(sea_orm_active_enums::TournamentPrivacyEnum)
                    .values(sea_orm_active_enums::TournamentPrivacyEnum.into_iter())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(users::Entity)
                    .if_not_exists()
                    .col(ColumnDef::new(users::Column::Id).primary_key())
                    .col(
                        ColumnDef::new(users::Column::Username)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(users::Column::Email)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(users::Column::Passhash).string().not_null())
                    .col(
                        ColumnDef::new(users::Column::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(users::Column::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(texts::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(texts::Column::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(texts::Column::Content).text().not_null())
                    .col(ColumnDef::new(texts::Column::Options).json_binary().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(tournaments::Entity)
                    .if_not_exists()
                    .col(ColumnDef::new(tournaments::Column::Id).primary_key())
                    .col(
                        ColumnDef::new(tournaments::Column::Title)
                            .string()
                            .char_len(1024)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(tournaments::Column::Description)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(tournaments::Column::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(tournaments::Column::CreatedBy)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(tournaments::Column::ScheduledFor)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(tournaments::Column::StartedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(tournaments::Column::EndedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(tournaments::Column::Privacy)
                            .enumeration(
                                sea_orm_active_enums::TournamentPrivacyEnum,
                                sea_orm_active_enums::TournamentPrivacyEnum.into_iter(),
                            )
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(tournaments::Column::TextOptions)
                            .json_binary()
                            .null(),
                    )
                    .col(ColumnDef::new(tournaments::Column::TextId).integer().null())
                    .col(
                        ColumnDef::new(tournaments::Column::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-tournament-created_by")
                            .from(tournaments::Entity, tournaments::Column::CreatedBy)
                            .to(users::Entity, users::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-tournament-text_id")
                            .from(tournaments::Entity, tournaments::Column::TextId)
                            .to(texts::Entity, texts::Column::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(otp::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(otp::Column::Email)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(otp::Column::Otp).integer().not_null())
                    .col(
                        ColumnDef::new(otp::Column::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-otp-email")
                            .from(otp::Entity, otp::Column::Email)
                            .to(users::Entity, users::Column::Email)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // 6. Create CompletedSessions Table (You'll need to define its columns and FKs)
        // manager.create_table(Table::create().table(completed_sessions::Entity)...).await?;
        // And add foreign keys from users and tournaments to completed_sessions if they are one-to-many.
        // Your current relations define completed_sessions as `has_many`, so completed_sessions would have `user_id` and `tournament_id` FKs.

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order of creation, and drop FKs implicitly with tables or explicitly if needed.
        // Drop custom enum type last.

        // manager.drop_table(Table::drop().table(completed_sessions::Entity).to_owned()).await?;
        manager
            .drop_table(Table::drop().table(otp::Entity).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(tournaments::Entity).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(texts::Entity).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(users::Entity).to_owned())
            .await?;

        // Drop the custom ENUM type (PostgreSQL specific)
        manager
            .drop_type(
                Type::drop()
                    .name(sea_orm_active_enums::TournamentPrivacyEnum)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

// In your `migration/src/lib.rs`, make sure this migration is added to the Migrator struct:
// pub struct Migrator;
//
// #[async_trait::async_trait]
// impl MigratorTrait for Migrator {
//     fn migrations() -> Vec<Box<dyn MigrationTrait>> {
//         vec![
//             Box::new(mYYYYMMDD_HHMMSS_your_migration_name::Migration),
//             // Add other migrations here
//         ]
//     }
// }
