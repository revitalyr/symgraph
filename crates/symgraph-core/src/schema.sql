
-- SQLite schema for symgraph
CREATE TABLE IF NOT EXISTS modules (
  id     INTEGER PRIMARY KEY,
  name   TEXT NOT NULL,
  kind   TEXT NOT NULL,
  path   TEXT
);

CREATE TABLE IF NOT EXISTS files (
  id        INTEGER PRIMARY KEY,
  module_id INTEGER,
  path      TEXT NOT NULL,
  lang      TEXT NOT NULL,
  FOREIGN KEY(module_id) REFERENCES modules(id)
);

CREATE TABLE IF NOT EXISTS symbols (
  id            INTEGER PRIMARY KEY,
  file_id       INTEGER NOT NULL,
  usr           TEXT,
  key           TEXT,
  name          TEXT NOT NULL,
  kind          TEXT NOT NULL,
  is_definition INTEGER NOT NULL,
  FOREIGN KEY(file_id) REFERENCES files(id)
);

CREATE TABLE IF NOT EXISTS occurrences (
  id         INTEGER PRIMARY KEY,
  symbol_id  INTEGER NOT NULL,
  file_id    INTEGER NOT NULL,
  usage_kind TEXT NOT NULL,
  line       INTEGER NOT NULL,
  column     INTEGER NOT NULL,
  FOREIGN KEY(symbol_id) REFERENCES symbols(id),
  FOREIGN KEY(file_id) REFERENCES files(id)
);

CREATE TABLE IF NOT EXISTS edges (
  id          INTEGER PRIMARY KEY,
  from_sym    INTEGER,
  to_sym      INTEGER,
  from_module INTEGER,
  to_module   INTEGER,
  kind        TEXT NOT NULL,
  FOREIGN KEY(from_sym) REFERENCES symbols(id),
  FOREIGN KEY(to_sym)   REFERENCES symbols(id),
  FOREIGN KEY(from_module) REFERENCES modules(id),
  FOREIGN KEY(to_module)   REFERENCES modules(id)
);
