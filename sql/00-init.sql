SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE
usename = 's3index' OR datname = 's3bucketfiles';
DROP DATABASE IF EXISTS s3bucketfiles;
DROP USER IF EXISTS s3index;

-- DEV ONLY - Dev only password (for local dev and unit test).
CREATE USER s3index PASSWORD 's3index_password';
CREATE DATABASE s3bucketfiles owner s3index ENCODING = 'UTF-8';
ALTER ROLE s3index WITH SUPERUSER;
ALTER ROLE s3index SET search_path = public;
-- Grant necessary permissions to the user
GRANT ALL PRIVILEGES ON DATABASE s3bucketfiles TO s3index;

-- If you have schemas, ensure the user can create tables in that schema
GRANT USAGE, CREATE ON SCHEMA public TO s3index;

-- If you want the user to have the ability to insert data
GRANT INSERT, SELECT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO s3index;