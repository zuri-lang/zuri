CREATE TABLE publishers (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  username VARCHAR(40) NOT NULL,
  username_key VARCHAR(40) COLLATE utf8mb4_bin NOT NULL UNIQUE,
  email VARCHAR(254) NOT NULL,
  email_key VARCHAR(254) COLLATE utf8mb4_bin NOT NULL UNIQUE,
  password_hash VARCHAR(255) NOT NULL,
  recovery_digest VARCHAR(64) NOT NULL,
  role VARCHAR(16) NOT NULL DEFAULT 'publisher',
  email_verified INTEGER NOT NULL DEFAULT 0,
  suspended_at BIGINT,
  created_at BIGINT NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

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
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE INDEX tokens_by_publisher ON tokens (publisher_id);

CREATE TABLE packages (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  name VARCHAR(64) NOT NULL,
  name_key VARCHAR(64) COLLATE utf8mb4_bin NOT NULL UNIQUE,
  description TEXT NOT NULL,
  homepage TEXT,
  repository TEXT,
  license VARCHAR(255),
  keywords TEXT NOT NULL,
  latest VARCHAR(255),
  downloads BIGINT NOT NULL DEFAULT 0,
  created_at BIGINT NOT NULL,
  updated_at BIGINT NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE owners (
  package_id BIGINT NOT NULL REFERENCES packages (id) ON DELETE CASCADE,
  publisher_id BIGINT NOT NULL REFERENCES publishers (id) ON DELETE CASCADE,
  added_at BIGINT NOT NULL,
  PRIMARY KEY (package_id, publisher_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE versions (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  package_id BIGINT NOT NULL REFERENCES packages (id) ON DELETE CASCADE,
  version VARCHAR(255) NOT NULL,
  version_key VARCHAR(255) COLLATE utf8mb4_bin NOT NULL,
  checksum VARCHAR(80) NOT NULL,
  size BIGINT NOT NULL,
  manifest MEDIUMTEXT NOT NULL,
  dependencies MEDIUMTEXT NOT NULL,
  zuri VARCHAR(255),
  readme MEDIUMTEXT,
  yanked_at BIGINT,
  published_by BIGINT REFERENCES publishers (id),
  published_at BIGINT NOT NULL,
  downloads BIGINT NOT NULL DEFAULT 0,
  UNIQUE (package_id, version_key)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE downloads_daily (
  version_id BIGINT NOT NULL REFERENCES versions (id) ON DELETE CASCADE,
  day VARCHAR(10) NOT NULL,
  downloads BIGINT NOT NULL,
  PRIMARY KEY (version_id, day)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE audit_log (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  at BIGINT NOT NULL,
  publisher_id BIGINT,
  action VARCHAR(32) NOT NULL,
  subject VARCHAR(255) NOT NULL,
  detail TEXT,
  address VARCHAR(64)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE login_failures (
  failure_key VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY,
  failures INTEGER NOT NULL,
  first_at BIGINT NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE one_time_codes (
  digest VARCHAR(64) PRIMARY KEY,
  publisher_id BIGINT NOT NULL REFERENCES publishers (id) ON DELETE CASCADE,
  purpose VARCHAR(16) NOT NULL,
  expires_at BIGINT NOT NULL,
  used_at BIGINT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
