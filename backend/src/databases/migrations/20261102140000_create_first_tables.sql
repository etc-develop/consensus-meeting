CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS meetings (
    id UUID PRIMARY KEY,
    meeting_no TEXT,
    meeting_time TIMESTAMPTZ,
    meeting_title TEXT,
    chairperson VARCHAR(100),
    attendees TEXT,
    meeting_location TEXT,
    recorder VARCHAR(100),
    title TEXT NOT NULL,
    summary TEXT,
    meeting_date DATE,
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS meeting_promotions (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    promotion_no INTEGER NOT NULL DEFAULT 1,
    content TEXT NOT NULL,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (meeting_id, promotion_no)
);

CREATE TABLE IF NOT EXISTS agenda_items (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    item_no TEXT NOT NULL,
    agenda_item TEXT NOT NULL,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (meeting_id, item_no)
);

CREATE TABLE IF NOT EXISTS resolutions (
    id UUID PRIMARY KEY,
    meeting_id UUID NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    agenda_item_id UUID NOT NULL UNIQUE REFERENCES agenda_items(id) ON DELETE CASCADE,
    item_no TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    resolution TEXT,
    result TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (meeting_id, item_no)
);

CREATE INDEX IF NOT EXISTS idx_meeting_promotions_meeting_id
    ON meeting_promotions (meeting_id);

CREATE INDEX IF NOT EXISTS idx_agenda_items_meeting_id
    ON agenda_items (meeting_id);

CREATE INDEX IF NOT EXISTS idx_resolutions_meeting_id
    ON resolutions (meeting_id);

CREATE TABLE IF NOT EXISTS documents (
    id UUID PRIMARY KEY,
    file_name TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    uploaded_by TEXT NOT NULL,
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    storage_path TEXT NOT NULL
);