-- Additive migration: adds folder-conflict metadata columns to conflict_records.
-- Existing rows default to is_dir=0 (false) and conflict_kind='content_modified'.
ALTER TABLE conflict_records ADD COLUMN is_dir INTEGER NOT NULL DEFAULT 0;
ALTER TABLE conflict_records ADD COLUMN conflict_kind TEXT NOT NULL DEFAULT 'content_modified';
