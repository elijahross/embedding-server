-- ENUMS
CREATE TYPE Role AS ENUM ('admin', 'viewer', 'inactive');
CREATE EXTENSION IF NOT EXISTS vector;

CREATE SEQUENCE files_file_id_seq START 1000;

CREATE TABLE Files (
    "file_id" BIGINT PRIMARY KEY DEFAULT nextval('files_file_id_seq'),
    "filename" TEXT NOT NULL,
    "applicant" TEXT NOT NULL,
    "content_md" TEXT,
    "embedding" vector(768),
    "uploaded_at" TIMESTAMP DEFAULT now()
);

CREATE TABLE Users (
    "user_id" VARCHAR PRIMARY KEY,
    "first_name" VARCHAR NOT NULL,
    "last_name" VARCHAR NOT NULL,
    "email" VARCHAR NOT NULL UNIQUE,
    "role" Role DEFAULT 'viewer',
    "api_key" TEXT UNIQUE,
    "salt" UUID DEFAULT gen_random_uuid(),
    "created_at" TIMESTAMP DEFAULT now()
);

CREATE INDEX idx_user_api_key ON Users ("api_key");
CREATE INDEX idx_user_email ON Users ("email");
CREATE INDEX idx_file_applicant ON Files ("applicant");
CREATE INDEX idx_file_filename ON Files ("filename");
CREATE INDEX idx_file_embedding ON Files USING ivfflat ("embedding");
CREATE INDEX idx_file_content_md_gin ON Files USING gin (to_tsvector('english', "content_md"));