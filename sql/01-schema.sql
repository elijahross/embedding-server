-- ENUMS
CREATE TYPE Role AS ENUM ('admin', 'viewer', 'inactive');
CREATE EXTENSION IF NOT EXISTS vector;

CREATE SEQUENCE files_file_id_seq START 1000;

CREATE TABLE Files (
    "file_id" BIGINT PRIMARY KEY DEFAULT nextval('files_file_id_seq'),
    "filename" TEXT NOT NULL,
    "applicant" TEXT NOT NULL,
    "file_type" TEXT NOT NULL,
    "created_at" TIMESTAMP DEFAULT now(),
    "processed" BOOLEAN DEFAULT FALSE
);

CREATE TABLE File_Chunks (
    "chunk_id" BIGSERIAL PRIMARY KEY,
    "file_id" BIGINT NOT NULL REFERENCES Files(file_id) ON DELETE CASCADE,
    "chunk_index" INT,
    "content_md" TEXT,
    "embedding" vector(768),
    "token_count" INT
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
CREATE INDEX idx_chunk_content_md_gin 
    ON File_Chunks USING gin (to_tsvector('english', "content_md"));
CREATE INDEX idx_chunk_embedding 
    ON File_Chunks USING ivfflat ("embedding" vector_cosine_ops)
    WITH (lists = 100); 
CREATE INDEX idx_chunk_file_order 
    ON File_Chunks ("file_id", "chunk_index");