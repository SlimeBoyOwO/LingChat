use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Role::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Role::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Role::ScriptKey).string_len(255))
                    .col(ColumnDef::new(Role::ScriptRoleKey).string_len(255))
                    .col(ColumnDef::new(Role::Name).string_len(255).not_null())
                    .col(
                        ColumnDef::new(Role::RoleType)
                            .string_len(32)
                            .not_null()
                            .default("npc"),
                    )
                    .col(ColumnDef::new(Role::ResourceFolder).string_len(512))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ix_role_name")
                    .table(Role::Table)
                    .col(Role::Name)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_script_role")
                    .table(Role::Table)
                    .col(Role::ScriptKey)
                    .col(Role::ScriptRoleKey)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Save::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Save::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Save::Title).string_len(255).not_null())
                    .col(ColumnDef::new(Save::Status).text().not_null().default("{}"))
                    .col(ColumnDef::new(Save::CreateDate).date_time().not_null())
                    .col(ColumnDef::new(Save::UpdateDate).date_time().not_null())
                    .col(ColumnDef::new(Save::RunningScriptId).integer())
                    .col(ColumnDef::new(Save::LastMessageId).integer())
                    .col(ColumnDef::new(Save::MainRoleId).integer())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Save::Table, Save::MainRoleId)
                            .to(Role::Table, Role::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(RunningScript::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RunningScript::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RunningScript::ScriptFolder)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RunningScript::VariableInfo)
                            .text()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(RunningScript::CurrentChapter)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RunningScript::EventSequence)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RunningScript::SaveId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(RunningScript::Table, RunningScript::SaveId)
                            .to(Save::Table, Save::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ix_running_script_folder")
                    .table(RunningScript::Table)
                    .col(RunningScript::ScriptFolder)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Line::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Line::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Line::Content).text().not_null())
                    .col(ColumnDef::new(Line::OriginalEmotion).string_len(64))
                    .col(ColumnDef::new(Line::PredictedEmotion).string_len(64))
                    .col(ColumnDef::new(Line::TtsContent).text())
                    .col(ColumnDef::new(Line::ActionContent).text())
                    .col(ColumnDef::new(Line::AudioFile).string_len(512))
                    .col(ColumnDef::new(Line::Attribute).string_len(32).not_null())
                    .col(ColumnDef::new(Line::SenderRoleId).integer())
                    .col(ColumnDef::new(Line::DisplayName).string_len(255))
                    .col(ColumnDef::new(Line::SaveId).integer().not_null())
                    .col(ColumnDef::new(Line::ParentLineId).integer())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Line::Table, Line::SaveId)
                            .to(Save::Table, Save::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Line::Table, Line::SenderRoleId)
                            .to(Role::Table, Role::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Line::Table, Line::ParentLineId)
                            .to(Line::Table, Line::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(LinePerception::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(LinePerception::LineId).integer().not_null())
                    .col(ColumnDef::new(LinePerception::RoleId).integer().not_null())
                    .primary_key(
                        Index::create()
                            .col(LinePerception::LineId)
                            .col(LinePerception::RoleId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(LinePerception::Table, LinePerception::LineId)
                            .to(Line::Table, Line::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(LinePerception::Table, LinePerception::RoleId)
                            .to(Role::Table, Role::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MemoryBank::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MemoryBank::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MemoryBank::Info)
                            .text()
                            .not_null()
                            .default("{}"),
                    )
                    .col(ColumnDef::new(MemoryBank::SaveId).integer().not_null())
                    .col(ColumnDef::new(MemoryBank::RoleId).integer())
                    .foreign_key(
                        ForeignKey::create()
                            .from(MemoryBank::Table, MemoryBank::SaveId)
                            .to(Save::Table, Save::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MemoryBank::Table, MemoryBank::RoleId)
                            .to(Role::Table, Role::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(AdventureUnlock::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AdventureUnlock::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AdventureUnlock::AdventureFolder)
                            .string_len(512)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(AdventureUnlock::CharacterFolder)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(ColumnDef::new(AdventureUnlock::UnlockedAt).date_time())
                    .col(ColumnDef::new(AdventureUnlock::CompletedAt).date_time())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ix_adventure_character")
                    .table(AdventureUnlock::Table)
                    .col(AdventureUnlock::CharacterFolder)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MemoryBank::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(AdventureUnlock::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(LinePerception::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Line::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RunningScript::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Save::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Role::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Role {
    Table,
    Id,
    ScriptKey,
    ScriptRoleKey,
    Name,
    RoleType,
    ResourceFolder,
}

#[derive(DeriveIden)]
enum Save {
    Table,
    Id,
    Title,
    Status,
    CreateDate,
    UpdateDate,
    RunningScriptId,
    LastMessageId,
    MainRoleId,
}

#[derive(DeriveIden)]
enum RunningScript {
    Table,
    Id,
    ScriptFolder,
    VariableInfo,
    CurrentChapter,
    EventSequence,
    SaveId,
}

#[derive(DeriveIden)]
enum Line {
    Table,
    Id,
    Content,
    OriginalEmotion,
    PredictedEmotion,
    TtsContent,
    ActionContent,
    AudioFile,
    Attribute,
    SenderRoleId,
    DisplayName,
    SaveId,
    ParentLineId,
}

#[derive(DeriveIden)]
enum LinePerception {
    Table,
    LineId,
    RoleId,
}

#[derive(DeriveIden)]
enum MemoryBank {
    Table,
    Id,
    Info,
    SaveId,
    RoleId,
}

#[derive(DeriveIden)]
enum AdventureUnlock {
    Table,
    Id,
    AdventureFolder,
    CharacterFolder,
    UnlockedAt,
    CompletedAt,
}
