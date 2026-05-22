import json
import hash
import template
import os
import log
import http.status
import .errors
import .db
import .template_ext
import .routes { routes }
import ..setup

def _log_request(req, res) {
  log.info('RepositoryAccess: ${req.ip} - "${req.method} ${req.request_uri} ' +
      'HTTP/${req.http_version}" ${res.status} ${res.status < 400 ? res.body.length() : '-'}')
}

def _get_uri_route_data(route, path) {
  var result = {}

  var name_pattern = '([a-zA-Z_][a-zA-Z0-9_]+)'
  var regex = route.
    # * in routes match anything or nothing after.
    replace('/[*]/', '(.*)').
    # + in routes match something after.
    replace('/[+]/', '(.+)').
    # convert {id} pattern to matching ids for path
    replace('/\\{${name_pattern}\\}/', '(?P<$1>[^/]+)')

  var matches = path.matches('~^${regex}$~i')
  if matches {
    for k, v in matches {
      result[k] = v[0]
    }
  }

  return result
}

def _start_session() {
  var key = hash.sha1(to_string(microtime()) + rand(111111111, 999999999))
  if db.create_session(key) > 0 {
    return key
  }
  return nil
}

def _setup_session(req, res) {
  res.session = {}
  res.session_key = nil
  if req.cookies.contains(setup.SESSION_NAME) {
    res.session_key = req.cookies[setup.SESSION_NAME]
    var ses = db.get_session(res.session_key)
    if is_dict(ses) {
      res.session.extend(ses)
    } else {
      res.session_key = _start_session()
      res.set_cookie(setup.SESSION_NAME, res.session_key)
    }
  } else {
    res.session_key = _start_session()
    res.set_cookie(setup.SESSION_NAME, res.session_key)
  }

  # bind '.clear_session()' to response
  res.clear_session = @{
    res.session = {}

    # just for simplicity
    return nil
  }
}

var _template_setup = @{
  var tpl = template()
  tpl.set_root(os.join_paths(setup.NYSSA_DIR, setup.TEMPLATES_DIR))
  for name, fn in template_ext() {
    tpl.register_function(name, fn)
  }
  return tpl.render
}()

def router(req, res) {
  catch {
    var view = errors.not_found

    # Check exact matches first...
    if routes.contains(req.path) {
      var data = routes[req.path]
      if data[0].upper() == req.method.upper()
        view = data[1]
    } else {
      for route, data in routes {
        var route_data = _get_uri_route_data(route, req.path)
        if route_data {
          if data[0].upper() == req.method.upper() {
            view = data[1]
            req.params = route_data
          }
        }
      }
    }

    # setup session
    _setup_session(req, res)

#     # bind '.json()' to response
#     res.json = @(v) {
#       res.headers['Content-Type'] = 'application/json'
#       res.write(json.encode(v))
#
#       # just for simplicity
#       return nil
#     }

    # bind the '.fail()' to response
    res.fail = @(code, v) {
      res.status = code
      res.headers['Content-Type'] = 'application/json'
      if !v v = status.map[code]
      res.json({error: v})

      # just for simplicity
      return nil
    }

    # setup shorthand for res.write(template())
    res.template = @(path, vars) {
      res.write(_template_setup(path, vars))
    }

    view(req, res)

    # update session
    if res.session_key {
      db.update_session(res.session_key, json.encode(res.session))
    }
  } as e

  if e {
    errors.server_error(e, req, res)
  }

  # Log every request before exiting the client session.
  _log_request(req, res)
}
