import args
import io
import os
import http
import json
import log
import ..setup

# read the state file
var state = json.parse(setup.STATE_FILE) or {}
if !is_dict(state) state = {}

def parse(parser) {
  parser.add_command(
    'account', 
    'Manages a Nyssa publisher account',
    {
      type: args.CHOICE,
      # choices: ['create', 'login', 'logout'],
      choices: {
        create: 'Creates a new publisher account',
        login: 'Login to a publisher account',
        logout: 'Log out of a publisher account',
      }
    }
  ).add_option(
    'repo', 
    'the repo where the account is located', 
    {
      short_name: 'r',
      type: args.OPTIONAL,
    }
  )
}

def create(repo, success, error) {
  # warn about account overwrite
  if state.get('name', nil) and state.get('key', nil) {
    var name = state['name']
    echo 'Account "${name}" currently logged in. If you continue, ${name} will be logged out.'
    if !['y', 'Y'].contains(io.readline('Do you want to continue? [y/N]').trim())
      return
  }

  var details = {
    name: io.readline('username:').trim(),
    email: io.readline('email:').trim(),
    password: io.readline('password:', true).trim(),
  }
  echo '' # because password prompt won't go to a new line.

  catch {
    log.info('Creating new publisher account at ${repo}.')
    var res = http.post('${repo}/api/create-publisher', details)
    var body = json.decode(res.body.to_string())

    if res.status == 200 {

      # update the state
      state['name'] = details.name
      state['email'] = details.email
      state['key'] = body.key
      file(state_file, 'w').write(json.encode(state, false))

      success(
        'Account created successfully!\n' +
        'Publisher Key: ${body.key}',
        'Save your key somewhere safe cause you\'ll need it to recover '+
        'your account if you ever forget your password.'
      )
    } else {
      error('Account creation failed:\n  ${body.error}')
    }
  } as e

  if e {
    error('Account creation failed:\n  ${e.message}')
  }
}

def login(repo, success, error) {
  # warn about account overwrite
  if state.get('name', nil) and state.get('key', nil) {
    var name = state['name']
    echo 'Account "${name}" currently logged in. If you continue, ${name} will be logged out.'
    if !['y', 'Y'].contains(io.readline('Do you want to continue? [y/N]').trim())
      return
  }

  var details = {
    username: io.readline('username:').trim(),
    password: io.readline('password:', true).trim(),
  }
  echo '' # because password prompt won't go to a new line.

  catch {
    log.info('Login in to publisher account at ${repo}.')
    var res = http.post('${repo}/api/login', details)
    var body = json.decode(res.body.to_string())

    if res.status == 200 {

      # update the state
      state['name'] = body.username
      state['email'] = body.email
      state['key'] = body.key
      
      file(state_file, 'w').write(json.encode(state, false))
      success(
        'Logged in as ${body.username} successfully!\n' +
        'Publisher Key: ${body.key}'
      )
    } else {
      error('Login failed:\n  ${body.error}')
    }
  } as e

  if e {
    error('Login failed:\n  ${e.message}')
  }
}

def logout(repo, success, error) {
  if state.get('name', nil)
    state.remove('name')
  if state.get('key', nil)
    state.remove('key')

  catch {
    if file(state_file, 'w').write(json.encode(state, false))
      success('Logged out of publisher account!')
  } as e

  if e {
    error('Login failed:\n  ${e.message}')
  }
}

def run(value, options, success, error) {
  var repo = options.get('repo', setup.DEFAULT_REPOSITORY)

  using value {
    when 'login' login(repo, success, error)
    when 'create' create(repo, success, error)
    when 'logout' logout(repo, success, error)
  }
}
