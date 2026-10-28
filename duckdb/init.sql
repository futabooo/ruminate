-- Views over the ruminate data repository.
-- `ruminate db` replaces the DATA_DIR placeholder with the clone's path and
-- starts DuckDB with this file. Column types are fixed here so every reader
-- (this file, the nightly tasks, your own queries) sees the same shape.

CREATE OR REPLACE VIEW messages AS
SELECT *
FROM read_json('{{DATA_DIR}}/logs/**/*.jsonl',
  format = 'newline_delimited',
  columns = {
    agent: 'VARCHAR',
    channel: 'VARCHAR',
    session: 'VARCHAR',
    title: 'VARCHAR',
    project: 'VARCHAR',
    git_branch: 'VARCHAR',
    url: 'VARCHAR',
    date: 'TIMESTAMPTZ',
    seq: 'INTEGER',
    role: 'VARCHAR',
    text: 'VARCHAR',
    actions: 'VARCHAR[]'
  });

CREATE OR REPLACE VIEW sessions AS
SELECT
  agent,
  session,
  any_value(title)   AS title,
  any_value(project) AS project,
  any_value(url)     AS url,
  min(date)          AS started_at,
  max(date)          AS ended_at,
  count(*) FILTER (WHERE role = 'user') AS prompts,
  count(*)           AS turns
FROM messages
GROUP BY agent, session;
