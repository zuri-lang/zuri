#!-- part of the sqlite module

import .exception { * }
import .cursor { * }
import _sqlite { 
  _open, 
  _close, 
  _exec, 
  _last_insert_id, 
  _query 
}

/**
 * SQLite3 management class
 */
class SQLite3 {

  /**
   * The path to the SQLite3 file
   * @default = :memory:
   */
  var path = ':memory:'

  # pointer to sqlite3 C struct
  var _db

  # tracks the open/closed state of the sqlite
  var _is_open = false

  /**
   * @param string path
   * @note The database doesn't need to exist.
   * @constructor
   */
  SQLite3(path) {
    if path != nil {
      if !is_string(path)
        raise SQLiteException('database path expected')

      self.path = path
    }
  }

  /**
   * Opens the handle to a database file 
   */
  open() {
    self._db = _open(self.path)

    if is_string(self._db)
      raise SQLiteException('could not open database: ${self._db}')

    self._is_open = true
  }

  /**
   * Closes the handle to the database and return `true` if successfully
   * closed or `false` otherwise.
   * 
   * @returns boolean
   */
  close() {
    if(_close(self._db)) {
      self._is_open = false
      return true
    }

    return false
  }

  /**
   * Executes a query string as is and returns `true` if the
   * query was executed or `false` otherwise.
   * 
   * @note this method does not return a query result
   * @note this method takes optional params like `query()` (see below).
   * @param string query
   * @param list|dict|nil params
   * @returns boolean
   * @throws [[sqlite.SQLiteException]] if an error occured
   */
  exec(query, params) {
    if !is_string(query)
      raise TypeError('string expected, ${typeof(query)} given')

    if !self._is_open
      raise SQLiteException('database not open for exec')

    var result = _exec(self._db, query, params)
    if is_string(result)
      raise SQLiteException('SQL error ${result}')

    return result
  }

  /**
   * The id of the last insert operation.
   * 
   * Returns: 
   * * `-1` if the last insert failed, 
   * * `0` if no insert statement has been executed or 
   * * A number greater than 0 if it succeeded
   * 
   * @returns number
   * @throws [[sqlite.SQLiteException]] if database is not opened
   */
  last_insert_id() {

    if !self._is_open
      raise SQLiteException('database not open for exec')

    return _last_insert_id(self._db)
  }

  /**
   * query(sql: string [, params: list | dict])
   * 
   * Executes and sql query and returns the result of the execution.
   * 
   * 1. Pass a list as _params_ if you have unnamed parameterized queries.
   * 
   * For example,
   * 
   * ```zuri
   * sqlite.query('SELECT * FROM users WHERE id = ? AND name = ?', [3, 'James'])
   * ```

   * 2. Or pass a dictionary as _params_ if you use named paramters
   * 
   * For Example,
   * 
   * ```zuri
   * sqlite.query(
   *   'SELECT * FROM user WHERE id = :id AND name = :name', 
   *   {':id': 1, ':name': 'James'}
   * )
   * ```
   * 
   * @param string sql
   * @param list|dict|nil params
   * @returns [[sqlite.SQLite3Cursor]]
   * @throws [[sqlite.SQLiteException]] if an error occured.
   */
  query(sql, params) {
    if params != nil {
      if !is_list(params) and !is_dict(params)
        raise SQLiteException('list of query parameters expected')
    }

    if !self._is_open
      raise SQLiteException('database not open for query')

    var result = _query(self._db, sql, params)
    if is_string(result) 
      raise SQLiteException('SQL error ${result}')
    
      # if no error occurs, the _query function always returns a valid cursor
    return SQLite3Cursor(self, result)
  }

  /**
   * Runs an SQL query and returns the result as a list of dictionaries.
   * 
   * @note if the result is empty or the query is not a SELECT, it returns an empty list.
   * @param string sql
   * @param list|dict|nil params
   * @returns list[dictionary]
   */
  fetch(sql, params) {
    var cursor = self.query(sql, params)
    var result = []
    if cursor.columns.length() > 0 {
      while cursor.has_next() {
        var entry = {}
        for col in cursor.columns {
          entry.add(col, cursor.get(col))
        }
        result.append(entry)
      }
    }
    return result
  }
}

