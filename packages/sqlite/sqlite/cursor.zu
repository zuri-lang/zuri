#!-- part of the sqlite module

import .exception { * }
import _sqlite {
  _cursor_columns,
  _cursor_changes,
  _cursor_has_next,
  _cursor_get,
  _cursor_close
}

/**
 * A cursor for navigation through sql results
 * @iterable
 */
class SQLite3Cursor {

  #  the cursor item
  var _cursor

  # tracks if this cursor is still open or not
  var _is_closed

  /**
   * The SQLite3 connection that owns this cursor
   * @readonly
   */
  var connection

  /**
   * The number of rows in the cursor
   * @readonly
   */
  var row_count = 0

  /**
   * This value hold the number of rows modified, inserted or deleted by the the query that 
   * owns this cursor provided the query is one of INSERT, UPDATE or DELETE statement.
   * Executing any other type of SQL statement does not change this value from 0.
   * 
   * Only changes made directly by the INSERT, UPDATE or DELETE statement are considered 
   * - auxiliary changes caused by triggers, foreign key actions or REPLACE constraint 
   * resolution are not counted.
   * 
   * Changes to a view that are intercepted by INSTEAD OF triggers are not counted. 
   * The value returned by `modified_count` immediately after an INSERT, UPDATE or DELETE 
   * statement run on a view is always zero. Only changes made to real tables are counted.
   * 
   * @readonly
   * 
   * > If a separate thread makes changes on the same database connection at the exact time 
   * > the original query was also making a change, the result of this value will become 
   * > undependable.
   */
  var modified_count = 0

  /**
   * A list of the columns available in the result set.
   * @readonly
   */
  var columns = []

  /**
   * @param [[sqlite.SQLite3]] db
   * @param ptr cursor
   * @note SQLite3Cursor should NEVER be maually instantiated.
   * @constructor
   */
  SQLite3Cursor(db, cursor) {
    self.connection = db
    self._cursor = cursor
    self.columns = _cursor_columns(cursor)
    self.modified_count = _cursor_changes(cursor)
  }

  /**
   * Closes the cursor and prevents further reading.
   * 
   * @returns bool
   */
  close() {
    return _cursor_close(self._cursor)
  }

  /**
   * Returns `true` if there are more rows in the result set not yet retrieved, 
   * otherwise it returns `false`.
   * 
   * @returns boolean
   */
  has_next() {
    var result = _cursor_has_next(self._cursor)

    if is_string(result)
      raise SQLiteException(result)

    return result
  }

  /**
   * Returns the value of the column matching the index in the current result set.
   * 
   * @note If index is a number, it returns the value in the column at the given index. 
   * @note Index must be lower than columns.length() in this case.
   * @note If index is a string, it returns the value in the column with the given name.
   * @param number|string index
   * @returns string
   * @throws [[sqlite.SQLiteException]] if no matching column can be found.
   */
  get(index) {
    if is_number(index) or is_string(index) {
      if is_number(index) {
        if index < 0  or index > self.columns.length()
          raise SQLiteException('unknown column index')
      } else if is_string(index) {
        index = self.columns.index_of(index)
        if index == -1
          raise SQLiteException('unknown column name')
      }

      return _cursor_get(self._cursor, index)
    }

    raise ValueError('column name or index expected')
  }

  /**
   * implementing iterable decorator @iter
   * 
   * if you will not be needing most of the columns in the result set 
   * (and you don't think you should review your query), then using a while 
   * loop might slightly perform better than a for loop.
   */
  @iter(n) {
    var result = {}
    for col in self.columns {
      result.add(col, self.get(col))
    }
    return result
  }

  /**
   * implementing iterable decorator @itern
   */
  @itern(n) {
    return self.has_next()
  }
}

