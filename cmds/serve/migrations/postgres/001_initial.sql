CREATE TABLE publishers (
  id BIGSERIAL PRIMARY KEY,
  username VARCHAR(40) NOT NULL,
  username_key VARCHAR(40) COLLATE "C" NOT NULL UNIQUE,
  email VARCHAR(254) NOT NULL,
  email_key VARCHAR(254) COLLATE "C" NOT NULL UNIQUE,
  password_hash VARCHAR(255) NOT NULL,
  recovery_digest VARCHAR(64) NOT NULL,
  role VARCHAR(16) NOT NULL DEFAULT 'publisher',
  email_verified INTEGER NOT NULL DEFAULT 0,
  suspended_at BIGINT,
  created_at BIGINT NOT NULL
);

CREATE TABLE tokens (
  id VARCHAR(32) PRIMARY KEY,
  publisher_id BIGINT NOT NULL REFERENCES publishers (id) ON DELETE CASCADE,
  name VARCHAR(64) NOT NULL,
  digest VARCHAR(64) NOT NULL UNIQUE,
  scopes VARCHAR(64) NOT NULL,
  created_at BIGINT NOT NULL,
  expires_at BIGINT,
  last_used_at BIGINT,
  revoked_at BIGINT
);

CREATE INDEX tokens_by_publisher ON tokens (publisher_id);

CREATE TABLE packages (
  id BIGSERIAL PRIMARY KEY,
  name VARCHAR(64) NOT NULL,
  name_key VARCHAR(64) COLLATE "C" NOT NULL UNIQUE,
  description TEXT NOT NULL DEFAULT '',
  homepage TEXT,
  repository TEXT,
  license VARCHAR(255),
  keywords TEXT NOT NULL DEFAULT '',
  latest VARCHAR(255),
  downloads BIGINT NOT NULL DEFAULT 0,
  created_at BIGINT NOT NULL,
  updated_at BIGINT NOT NULL
);

CREATE TABLE owners (
  package_id BIGINT NOT NULL REFERENCES packages (id) ON DELETE CASCADE,
  publisher_id BIGINT NOT NULL REFERENCES publishers (id) ON DELETE CASCADE,
  added_at BIGINT NOT NULL,
  PRIMARY KEY (package_id, publisher_id)
);

CREATE TABLE versions (
  id BIGSERIAL PRIMARY KEY,
  package_id BIGINT NOT NULL REFERENCES packages (id) ON DELETE CASCADE,
  version VARCHAR(255) NOT NULL,
  version_key VARCHAR(255) COLLATE "C" NOT NULL,
  checksum VARCHAR(80) NOT NULL,
  size BIGINT NOT NULL,
  manifest TEXT NOT NULL,
  dependencies TEXT NOT NULL,
  zuri VARCHAR(255),
  readme TEXT,
  yanked_at BIGINT,
  published_by BIGINT REFERENCES publishers (id),
  published_at BIGINT NOT NULL,
  downloads BIGINT NOT NULL DEFAULT 0,
  UNIQUE (package_id, version_key)
);

CREATE TABLE downloads_daily (
  version_id BIGINT NOT NULL REFERENCES versions (id) ON DELETE CASCADE,
  day VARCHAR(10) NOT NULL,
  downloads BIGINT NOT NULL,
  PRIMARY KEY (version_id, day)
);

CREATE TABLE audit_log (
  id BIGSERIAL PRIMARY KEY,
  at BIGINT NOT NULL,
  publisher_id BIGINT,
  action VARCHAR(32) NOT NULL,
  subject VARCHAR(255) NOT NULL,
  detail TEXT,
  address VARCHAR(64)
);

CREATE TABLE login_failures (
  failure_key VARCHAR(255) COLLATE "C" PRIMARY KEY,
  failures INTEGER NOT NULL,
  first_at BIGINT NOT NULL
);

CREATE TABLE one_time_codes (
  digest VARCHAR(64) PRIMARY KEY,
  publisher_id BIGINT NOT NULL REFERENCES publishers (id) ON DELETE CASCADE,
  purpose VARCHAR(16) NOT NULL,
  expires_at BIGINT NOT NULL,
  used_at BIGINT
);
