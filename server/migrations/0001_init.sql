CREATE TABLE IF NOT EXISTS votes (
  id serial PRIMARY KEY,
  account VARCHAR NOT NULL,
  option SMALLINT NOT NULL,
  proposal_id BIGINT NOT NULL,
  dao VARCHAR NOT NULL
);
