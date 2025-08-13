-- Seed data for Users table

INSERT INTO Users ("user_id", "first_name", "last_name", "email", "role", "api_key")
VALUES
    ('u1', 'Alice', 'Smith', 'alice.smith@example.com', 'admin', 'api_key_1'),
    ('u2', 'Bob', 'Jones', 'bob.jones@example.com', 'viewer', 'api_key_2'),
    ('u3', 'Carol', 'Taylor', 'carol.taylor@example.com', 'inactive', 'api_key_3');

INSERT INTO files (file_id, applicant, filename, content_md, embedding, uploaded_at)
VALUES
    (
        'file_001',
        'applicant_123',
        'resume.pdf',
        '# John Doe Resume\n\nExperience: Rust Developer...',
        NULL,
        NOW()
    ),
    (
        'file_002',
        'applicant_123',
        'portfolio.pdf',
        '# Portfolio\n\nThis is a markdown version of my portfolio.',
        NULL,
        NOW()
    ),
    (
        'file_003',
        'applicant_456',
        'cover_letter.pdf',
        '# Cover Letter\n\nDear Hiring Manager...',
        NULL,
        NOW()
    ),
    (
        'file_004',
        'applicant_456',
        'project_doc.pdf',
        '# Project Documentation\n\n## Overview\n\nProject description here...',
        NULL,
        NOW()
    );