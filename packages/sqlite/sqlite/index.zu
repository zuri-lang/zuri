/**
 * @module sqlite
 *
 * SQLite is a lightweight disk/memory based database that does not require
 * a server or third party applications. It allows users to work with the
 * database using its sigtly variant version of the Structured Query Lanaguage.
 * For this reason, it is a perfect fit for embeded and highly portable database
 * applications. It can be used as a prototyping database from which an application
 * can be migrated to a more robust database system such as Oracle.
 *
 * This module provides an interface to working with SQLite databases and is
 * compartible with SQLite3.
 *
 * The `open()` function is the entry point to this module and must be called to create
 * a valid SQLite3 connection to a valid database. The following example shows how to
 * create a connection to a database.
 *
 * ```zuri
 * import sqlite
 * var con = sqlite.open('test.db')
 * ```
 *
 * The above code opens a connection to the database file `test.db` in the current directory.
 * Any valid file path is acceptable here. You can also open a kind of connection a database
 * that is stored completely in virtual memory, allowing you to use SQLite like an in-memory
 * database (albeit relational). The following example shows how to open a that make SQLite
 * behave like an in-memory database.
 *
 * ```zuri
 * import sqlite
 * var con = sqlite.open()
 * ```
 *
 * Once a connection has been established, you can use the connection to run all sorts of
 * queries. For example, you can run queries that do not return a result set (for example,
 * a `CREATE TABLE` query) using the `exec()` function as shown in the example below.
 *
 * ```zuri
 * # Create a new table
 * con.exec('CREATE TABLE users (id integer primary key, name text, gender text)')
 *
 * # Insert data into the table
 * # This isn't exactly the most optimal way to do it but you get the idea. Right?
 * con.exec('INSERT INTO users (id, name, gender) VALUES (0, "James", "Male")')
 * con.exec('INSERT INTO users (id, name, gender) VALUES (1, "Lilith", "Female")')
 * con.exec('INSERT INTO users (id, name, gender) VALUES (2, "Candy", "Non-Binary")')
 * ```
 *
 * This function will return `true` if the query was successful or `false` if it failed.
 *
 * You can retrieve the ID of the last insert query in the above command for example using
 * the `last_insert_id()` function. For example,
 *
 * ```zuri
 * con.last_insert_id()
 * # 2
 * ```
 *
 * On the other hand, there are two ways to run queries that return a dataset.
 *
 * ### Using the `query()` method.
 *
 * This function returns a `SQLite3Cursor` that allows you iterate through the dataset
 * and do as you wish with them. For example,
 *
 * ```zuri
 * var result = con.query('SELECT * FROM users')
 * ```
 *
 * There are two ways to loop through this result set. The first way is to use the `has_next()`
 * function. This function automatically moves the cursor to the next datarow in the result set
 * and return `true` or `false` when there are no more rows in the result set.
 *
 * ```zuri
 * while result.has_next() {
 *   var name = result.get(1)
 *   var gender = result.get(2)
 *   echo 'Name = ${name}, Gender = ${gender}'
 * }
 *
 * # ---- result ----------
 * Name = James, Gender = Male
 * Name = Lilith, Gender = Female
 * Name = Candy, Gender = Non-Binary
 * ```
 *
 * Once `has_next()` returns true, you'll be able to get the value of the different columns in
 * the result run using the `get()` method of the SQLite3Cursor by passing their ordinal position
 * as an argument to the function.
 *
 * Another way to get the result entries in a SQLite3Cursor is obviously using the `for` loop as
 * the class implements the _iterable_ decorators (as indicated in the class documentation below).
 * For example,
 *
 * ```zuri
 * for row in result {
 *   echo 'Name = ${row.name}, Gender = ${row.gender}'
 * }
 * # ---- result ----------
 * Name = James, Gender = Male
 * Name = Lilith, Gender = Female
 * Name = Candy, Gender = Non-Binary
 * ```
 *
 * > Much shorter right? Care should be taken though as a few of our tests have shown that for
 * > result sets with a large number of columns, the first option (using `while`) might be slightly
 * > faster for performace critical applications. However, no realworld dataset has been testd.
 *
 * ### Using the `fetch()` method.
 *
 * The second way to run queries that return a result set is to use the `fetch()` function.
 * Unlike the `query()` function that allows you to lazily access the resultset of a SQL query,
 * the `fetch()` function retrieves all results into a dictionary as a flat object. This function
 * is useful for returning all the data in the resultset.
 *
 * For example,
 *
 * ```zuri
 * con.fetch('SELECT * FROM users')
 *
 * # ---- result ---------
 * [
 *   {
 *     id: 0,
 *     name: James,
 *     gender: Male
 *   },
 *   {
 *     id: 1,
 *     name: Lilith,
 *     gender: Female
 *   },
 *   {
 *     id: 2,
 *     name: Candy,
 *     gender: Non-Binary
 *   }
 * ]
 * ```
 *
 * ### Parameterized Queries
 *
 * This module provides support for parameterized queries and as such offer protection against
 * SQL injection. An example of a parameterized query is show below.
 *
 * ```zuri
 * %> con.fetch('SELECT * FROM users WHERE name = ?', [ 'James' ])
 *
 * # ---- result ---------
 * [
 *   {
 *     id: 0,
 *     name: James,
 *     gender: Male
 *   }
 * ]
 * ```
 *
 * You can also used a dictionary as an argument instead of a list for named parameterized
 * queries. When you do this, the order or count of the parameters will not matter. Instead,
 * parameters will be matched based on their value in the dictionary. For example,
 *
 * ```zuri
 * con.fetch(
 *   'select * from users where name = :name and id = :id',
 *   {
 *     ':id': 0,
 *     ':name': 'James',
 *   }
 * )
 *
 * # ---- result ---------
 * [
 *   {
 *     id: 0,
 *     name: James,
 *     gender: Male
 *   }
 * ]
 * ```
 *
 * It is also a very good practice to always close your connection once done with it.
 * This is really simple.
 *
 * ```zuri
 * con.close()
 * ```
 *
 * _See below for more info_
 *
 * @copyright 2021, Richard Ore and Zuri contributors
 */

import .sqlite3 { * }

/**
 * Returns an handle to a sqlite3 database. If _path_ is not given, 
 * it will create an in-memory sqlite database.
 * 
 * @param string? path
 * @returns [[sqlite.SQLite3]]
 */
def open(path) {
  var sqlite = SQLite3(path)
  sqlite.open()
  return sqlite
}

