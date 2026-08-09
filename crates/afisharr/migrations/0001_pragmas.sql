-- no-transaction
-- SPDX-FileCopyrightText: 2026 Afisharr contributors
-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- 0001 — the one-way doors, set before the first CREATE TABLE (PRD §19.3).
--
-- These three cannot be set after the file has been written to. They are their
-- own migration because they cannot run inside a transaction, and `-- no-transaction`
-- is per-file: keeping the DDL in 0002 lets it stay transactional, so a
-- migration that fails part-way still rolls back cleanly.
--
-- `page_size` and `auto_vacuum` are fixed by the file's *first* write, and
-- `sqlx migrate` creates its own bookkeeping table before it runs anything
-- here. The connection options in `afisharr_core::storage` therefore apply the
-- same three values at file creation; these statements are what makes the
-- choice visible where a later reader looks for it, and `journal_mode` is
-- genuinely applied by this one.

PRAGMA page_size = 8192;
PRAGMA auto_vacuum = INCREMENTAL;
PRAGMA journal_mode = WAL;
