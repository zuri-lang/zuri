#!-- part of the curl module

import _curl
import .infos { Info }


/**
 * cURL Mime object for multipart-data forms and POST requests.
 */
class CurlMime {

  /**
   * @param [[curl.Curl]] curl
   * @constructor
   */
  CurlMime(curl) {
    if !instance_of(curl, Curl)
      raise TypeError('instance of Curl expected')
    self._ptr = _curl.mime_init(curl.get_pointer())
  }

  /**
   * Adds a new mime part with the given name and value.
   * 
   * @param string name
   * @param any value
   * @returns bool
   */
  add(name, value) {
    if !is_string(name)
      raise TypeError('name must be string')

    # This allows us to benefit from to_string decorators.
    if !is_string(value) value = to_string(value)

    var part = _curl.mime_addpart(self._ptr)
    if _curl.mime_name(part, name)
      return _curl.mime_data(part, value)
    return false
  }

  /**
   * Adds a new mime part with the given data and type.
   * 
   * @param any value
   * @param string type
   * @returns bool
   */
  add_as(value, type) {
    if !is_string(type)
      raise TypeError('type must be string')

    # This allows us to benefit from to_string decorators.
    if !is_string(value) value = to_string(value)

    var part = _curl.mime_addpart(self._ptr)
    if _curl.mime_type(part, type)
      return _curl.mime_data(part, value)
    return false
  }

  /**
   * Adds a new mime part with the given data.
   * 
   * @param any data
   * @returns bool
   */
  add_data(data) {
    # This allows us to benefit from to_string decorators.
    if !is_string(data) data = to_string(data)

    var part = _curl.mime_addpart(self._ptr)
    return _curl.mime_data(part, data)
  }

  /**
   * Adds a new mime part with the given name and file.
   * 
   * @param string name
   * @param string|instance value
   * @returns bool
   */
  add_file(name, value) {
    if !is_string(name)
      raise TypeError('name must be string')

    # This allows us to benefit from to_string decorators.
    if !is_string(value) value = to_string(value)

    var part = _curl.mime_addpart(self._ptr)
    if _curl.mime_name(part, name)
      return _curl.mime_filedata(part, value)
    return false
  }

  /**
   * Adds a new mime subpart with the given mime.
   * 
   * @param [[curl.CurlMime]] mime
   * @param string type
   * @returns bool
   */
  add_mime(mime, type) {
    if !instance_of(mime, CurlMime)
      raise TypeError('mime must be an instance of CurlMime')
    if !is_string(type)
      raise TypeError('type must be string')

    var part = _curl.mime_addpart(self._ptr)
    if _curl.mime_subparts(part, mime.get_pointer())
      return _curl.mime_type(part, type)
    return false
  }

  /**
   * Sets the encoding with which the mime will be transfered.
   * 
   * @param string encoding
   */
  set_encoding(encoding) {
    if !is_string(encoding)
      raise TypeError('encoding must be string')
    _curl.mime_encoding(self._ptr)
  }

  /**
   * Returns the raw pointer object to the underlying libcurl mime implementation.
   */
  get_pointer() {
    return self._ptr
  }
}


/**
 * cURL list interface.
 */
class CurlList {

  /**
   * @param list[string] items
   * @constrctor
   */
  CurlList(items) {
    if !is_list(items)
      raise TypeError('list expected')

    self._ptr = _curl.slist_create(items)
    if self._ptr == nil 
      raise Exception('CurlList initialization failed')
  }

  /**
   * Close and disposes the pointer to the list
   */
  close() {
    _curl.slist_free(self._ptr)
  }

  /**
   * Returns the raw pointer object to the underlying libcurl list implementation.
   */
  get_pointer() {
    return self._ptr
  }
}


/**
 * cURL class
 */
class Curl {

  /**
   * @constructor
   */
  Curl() {
    self._ptr = _curl.easy_init()
  }

  /**
   * This function is used to tell `curl` how to behave. By setting the
   * appropriate options, the application can change `curl`'s behavior.  
   * All options are set with an option followed by a parameter. That parameter
   * can be a number, boolean, string, or an object pointer, depending on what 
   * the specific option expects. Read this `cURL` manual carefully as bad input 
   * values may cause `curl` to behave badly!  You can only set one option in each 
   * function call. A typical application uses many `set_option()` calls in the 
   * setup phase.
   * 
   * Options set with this function call are valid for all forthcoming
   * transfers performed using this instance.  The options are not in any way
   * reset between transfers, so if you want subsequent transfers with
   * different options, you must change them between the transfers. You can
   * optionally reset all options back to internal default with `reset()`.
   * 
   * @note Strings passed to `curl` as arguments, must not exceed 8MB in size.
   * @note The order in which the options are set does not matter.
   * @param Option option
   * @param any value
   * @returns boolean
   */
  set_option(option, value) {

    # Use direct pointers for CurlList and CurlMime
    if is_instance(value) {
      if instance_of(value, CurlList) or instance_of(value, CurlMime)
        value = value.get_pointer()
    }

    return _curl.easy_setopt(self._ptr, option, value)
  }

  /**
   * Requests internal information from the `curl` session with this function.
   * Use this function AFTER performing a transfer if you want to get transfer 
   * related data.
   * 
   * @param [[curl.Info]] info
   * @returns string|number|list
   */
  get_info(info) {
    return _curl.easy_getinfo(self._ptr, info)
  }

  /**
   * This function converts the given input string to a URL encoded string and
   * returns that as a new allocated string. All input characters that are not
   * a-z, A-Z, 0-9, '-', '.', '_' or '~' are converted to their "URL escaped"
   * version (%NN where NN is a two-digit hexadecimal number).
   * 
   * @note This function does not accept a strings longer than 8MB.
   * @param string str
   * @returns string
   */
  escape(str) {
    return _curl.easy_escape(self._ptr, str)
  }

  /**
   * This function converts the given URL encoded input string to a "plain
   * string" and returns that in an allocated memory area. All input characters 
   * that are URL encoded (%XX where XX is a two-digit hexadecimal number) are 
   * converted to their decoded versions.
   * 
   * @param string str
   * @returns string
   */
  unescape(str) {
    return _curl.easy_unescape(self._ptr, str)
  }

  /**
   * Performs the entire request in a blocking manner and returns when done, or 
   * if it failed. It returns a dictionary containing the `headers` and `body` key.
   * @returns dict
   * 
   * > You must never call this function simultaneously from two places using
   * > the same instance. Let the function return first before invoking it
   * > another time.
   * 
   * @returns dictionary
   */
  send() {
    return _curl.easy_perform(self._ptr)
  }

  /**
   * Re-initializes the instace to the default values. This puts back the
   * instance to the same state as it was in when it was just created.
   * 
   * > It keeps live connections, the Session ID cache, the DNS cache, the
   * > cookies , the shares or the alt-svc cache.
   */
  reset() {
    _curl.easy_reset(self._ptr)
  }

  /**
   * Closes the current Curl instance.
   * 
   * This might close all connections this instance has used and possibly has
   * kept open until now - unless it was attached to a multi handle while
   * doing the transfers. Don't call this function if you intend to transfer
   * more files, re-using Curl instances is a key to good performance.
   * 
   * @note Calling a function on the instance after this function has been called is illegal
   */
  close() {
    _curl.easy_cleanup(self._ptr)
  }

  /**
   * Returns the raw pointer object to the underlying libcurl.
   */
  get_pointer() {
    return self._ptr
  }
}
