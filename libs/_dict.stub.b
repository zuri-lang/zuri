/**
 * # Dictionaries
 * 
 * Zuri dictionaries are built on a powerful hashtable with fast key lookups. Creating Zuri dictionaries
 * are extremely similar to how you create objects in JavaScript and for developers coming from that
 * language will find Zuri dictionaries very familiar.
 * 
 * ## Constructing a dictionary
 * 
 * Zuri dictionaries are arbitrary key-value pairs separated by colons (`:`) enclosed in braces (`{}`).
 * 
 * For example:
 * 
 * ```zuri-repl
 * %> {'name': 'Zuri', 'version': 0.5}
 * {name: Zuri, version: 0.5}
 * %> {}    # empty dictionary
 * {}
 * ```
 * 
 * > While dictionary values can be any valid Zuri object, keys can only be one of [String](./strings),
 * > [Number](./numbers) or Boolean.
 * 
 * For example:
 * 
 * ```zuri-repl
 * %> {0: 'number', false: 'boolean', 'key': 'string'}
 * {0: number, false: boolean, key: string}
 * ```
 * 
 * For string keys in a dictionary, the quotation may be excluded given that the string contains no space 
 * or any non alphanumeric character or the unquoted version equals to a boolean value.
 * 
 * For example:
 * 
 * ```zuri-repl
 * %> {country: 'Nigeria', dial_code: 234, in_africa: true}
 * {country: Nigeria, dial_code: 234, in_africa: true}
 * ```
 * 
 * ## Automatic value assignment
 * 
 * Sometimes when creating a dictionary, it is very common to have keys that already match the name of a 
 * variable that already exists in scope. For example:
 * 
 * ```zuri-repl
 * %> var name = 'Paulina'
 * %> var my_dict = {name: name}
 * %> my_dict
 * {name: Paulina}
 * ```
 * 
 * Because of how common and frequent developers do this, Zuri has an automatic value assignment feature for
 * dictionaries that allow you to assign variables to dictionary keys if they are already within scope without
 * rewriting them.
 * 
 * For example,
 * 
 * ```zuri-repl
 * %> var name = 'Kagawa'
 * %> {name}
 * {name: Kagawa}
 * ```
 * 
 * ## Accessing members of a Dictionary
 * 
 * Each key in a dictionary is a property of the given dictionary and as such, dictionary like strings 
 * and lists support index access.
 * 
 * For example:
 * 
 * ```zuri-repl
 * %> var a = {name: 'Zuri', version: 0.5}
 * %> a['name']
 * 'Zuri'
 * ```
 * 
 * And they also support property access whenever the key is a string. For example, the above `a['name']` 
 * could also be written as:
 * 
 * ```zuri-repl
 * %> a.name
 * 'Zuri'
 * ```
 * 
 * ## Looping through dictionaries
 * 
 * There are two ways to loop through a dictionary in Zuri. We can loop through the dictionary itself
 * using the specialized _for_ loop, of loop through its keys using any of the _while_ or _iter_ loop.
 * 
 * Below is an example looping through the dictionary itself using the _for_ loop.
 * 
 * ```zuri-repl
 * %> var person = {name: 'Ceaser', birth: '29AD', nation: 'Rome', position: 'Emperor'}
 * %> for x, y in person {
 * ..   print(x, '=', y)
 * .. }
 * name = Ceaser
 * birth = 29AD
 * nation = Rome
 * position = Emperor
 * ```
 * 
 * Unlike strings and lists, note that this _for_ loop takes two variables. This is to allow us take 
 * the key into the first and the value into the second variable respectively. 
 * 
 * If we only want to loop through the values without concern for the keys, we can use a single variable 
 * to hold our value and still iterate correctly. Note however that the corresponding _key_ information 
 * will be lost in the loop.
 * 
 * For example:
 * 
 * ```zuri-repl
 * %> for x in person {
 * ..   echo x
 * .. }
 * 'Ceaser'
 * '29AD'
 * 'Rome'
 * 'Emperor'
 * ```
 * 
 * The following example loops through the dictionary by looping through the keys of the dictionary. 
 * 
 * ```zuri-repl
 * %> var keys = person.keys()
 * %> iter var i = 0; i < keys.length(); i++ {
 * ..   print(keys[i], '=', person[keys[i]])
 * .. }
 * name = Ceaser
 * birth = 29AD
 * nation = Rome
 * position = Emperor
 * ```
 * 
 * The technique presented above is to walk through the keys of the dictionary and use the current key 
 * to index the dictionary. While the example uses the _iter_ loop, the same technique applies to the 
 * _while_ loop. 
 */

class dict {

  /**
   * Returns the length of the dictionary. The length of a Zuri dictionary is equal to the number of
   * keys it contains. i.e. `dict.length() == dict.keys().length()`.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> {name: 'Zuri', version: 1}.length()
   * 2
   * ```
   * 
   * @return number
   */
  length() {}


  /**
   * Adds a new key-value pair to the dictionary with the given key and value.<br>
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> var dict = {}
   * %> dict.add('name', 'Zuri')
   * %> dict
   * {name: Zuri}
   * ```
   * 
   * @param {string} key
   * @param {any} value
   */
  add(key, value) {}


  /**
   * Sets the value of the given key to the given value in the dictionary. If there is no existing entry
   * for the key in the dictionary, a new entry will be added.<br>
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> dict.set('name', 'New Zuri')
   * %> dict
   * {name: New Zuri}
   * %> dict.set('version', 1)
   * %> dict
   * {name: New Zuri, version: 1}
   * ```
   * 
   * > **_@note_:** `dict.set(x, y)` is equivalent to the following Zuri code.
   * > ```zuri-repl
   * > %> if dict.contains(x) {
   * > ..   dict[x] = 1
   * > .. } else {
   * > ..   dict.add(x, 1)
   * > .. }
   * > ```
   * 
   * @param {string} key
   * @param {any} value
   */
  set(key, value) {}


  /**
   * Clears the content of the dictionary.<br>
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> var a = {name: 'Zuri'}
   * %> a
   * {name: Zuri}
   * %> a.clear()
   * %> a
   * {}
   * ```
   */
  clear() {}


  /**
   * Returns a new dictionary which is a deep copy of the original dictionary.<br>
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> var new_dict = dict.clone()
   * %> new_dict
   * {name: New Zuri, version: 1}
   * ```
   * 
   * @return {dict}
   */
  clone() {}


  /**
   * Returns a new dictionary that contains every key-value pair in the original dictionary except
   * for keys whose associated value is `nil`.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> var dict2 = {name: 'James', age: 20, address: nil, country: nil}
   * %> dict2.compact()
   * {name: James, age: 20}
   * ```
   * 
   * @return {dict}
   */
  compact() {}


  /**
   * Returns `true` if any of the keys in the dictionary is equal to _x_, `false` otherwise.<br>
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> dict2.contains('name')
   * true
   * %> dict2.contains('street')
   * false
   * ```
   * 
   * @param {string} key
   * @return boolean
   */
  contains(key) {}


  /**
   * Adds all key-value pairs in dictionary _x_ to the original dictionary.<br>
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> var dict = {name: 'Zuri'}
   * %> dict.extend({version: 1})
   * %> dict
   * {name: Zuri, version: 1}
   * ```
   * 
   * @param {dict} dict
   */
  extend(dict) {}


  /**
   * Returns the value of the given _key_ in the dictionary. If the given key is not defined in the
   * dictionary and the _default_ value is given, the default value will be returned. Otherwise, `nil`
   * is returned.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> dict.get('version')   # value exists
   * 1
   * %> dict.get('age')   # value does not exist
   * %> dict.get('age', 6)   # value does not exist, but default is given
   * 6
   * %> dict.get('version', 1.1)   # value exists and default is given
   * 1
   * ```
   * 
   * @param {string} key
   * @param {any|nil} default_value
   * @return {any|nil}
   */
  get(key, default_value) {}


  /**
   * Returns a list containing the keys in the dictionary.<br>
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> dict.keys()
   * [name, version]
   * ```
   * 
   * @return {list}
   */
  keys() {}


  /**
   * Returns a list containing the value of all keys in the dictionary.<br>
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> dict.values()
   * [Zuri, 1]
   * ```
   * 
   * @return {list}
   */
  values() {}


  /**
   * Removes a given key and it's corresponding value from the dictionary and returns the value of
   * the key.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> dict = {username: 'james', email: 'a@b.c', active: true}
   * %> dict.remove('active')
   * true
   * %> dict
   * {username: james, email: a@b.c}
   * ```
   * 
   * @param {string} key
   * @return {any|nil}
   */
  remove(key) {}


  /**
   * Returns `true` if the dictionary is empty, otherwise returns `false`.<br>
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> dict.is_empty()
   * false
   * %> {}.is_empty()
   * true
   * ```
   * 
   * @return {boolean}
   */
  is_empty() {}


  /**
   * Returns the key whose value is equal to _x_ in the dictionary or `nil` if no key has the value _x_.<br>
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> dict.find_key('james')
   * 'username'
   * %> dict.find_key('camel')
   * ```
   * 
   * @param {any} value
   * @return {string|nil}
   */
  find_key(value) {}


  /**
   * Returns a list that contains a list of key and a list of values from the dictionary. <br>
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> var dict = {username: 'james', email: 'a@b.c'}
   * %> dict.to_list()
   * [[username, email], [james, a@b.c]]
   * ```
   * 
   * @return {list}
   */
  to_list() {}


  @iter(n) {}
  @itern(n) {}
}
