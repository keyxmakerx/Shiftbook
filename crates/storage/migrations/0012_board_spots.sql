-- Boards (round 12): each board with a canvas of its own (a project, a meeting series, the
-- Daily notes, a folder) keeps where the owner put each card on it, apart from the Desk.
-- `board` is the board's key from `shiftbook_core::boards::Board::key`. Same conventions as 0001.
CREATE TABLE board_cards (
    id          TEXT PRIMARY KEY NOT NULL,
    owner_id    TEXT NOT NULL REFERENCES users (id),
    board       TEXT NOT NULL,
    record_kind TEXT NOT NULL,
    record_id   TEXT NOT NULL,
    x           INTEGER NOT NULL,
    y           INTEGER NOT NULL,
    z           INTEGER NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT
);
CREATE UNIQUE INDEX ux_board_cards_record ON board_cards (owner_id, board, record_kind, record_id)
    WHERE deleted_at IS NULL;
