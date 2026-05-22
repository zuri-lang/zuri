import args
import os
import json
import http
import colors
import zip
import log
import ..setup
import ..config { Config }

var storage_dir = os.join_paths(setup.NYSSA_DIR, setup.STORAGE_DIR)

def italics(t) {
  return colors.text(t, colors.style.italic)
}

def parse(parser) {
  parser.add_command(
    'publish', 
    'Publishes a Zuri package to a repository'
  ).add_option(
    'repo', 
    'repository url', 
    {
      short_name: 'r',
      type: args.OPTIONAL,
    }
  )
}

def get_files(root) {
  var result = []
  if root {
    for f in os.read_dir(root) {
      if f != '.' and f != '..' {
        var path = os.join_paths(root, f)
        if os.dir_exists(path) {
          result.extend(get_files(path))
        } else {
          result.append(path)
        }
      }
    }
  }
  return result
}

def copy_to_tmp(root, dest) {
  var files = get_files(root).filter(@(x) {
    return x != '.' and x != '..' and 
      !x.match('/(\\/|\\\\)[.]git\\1/')
  })

  for f in files {
    var dir = os.join_paths(dest, os.dir_name(f).ltrim('.'))
    if !os.dir_exists(dir) os.create_dir(dir)
    file(f).copy(os.join_paths(dest, f.ltrim('.')))
  }
}

def run(value, options, success, error) {
  var repo = options.get('repo', setup.DEFAULT_REPOSITORY),
      state_file = os.join_paths(setup.NYSSA_DIR, setup.STATE_FILE),
      config_file = os.join_paths(os.cwd(), setup.CONFIG_FILE),
      readme_file = os.join_paths(os.cwd(), setup.README_FILE),
      tmp_dest

  catch {
    log.info('Checking for valid publisher account')
    var state = json.decode(file(state_file).read().trim()  or '{}')
    if !state.get('name', nil) or !state.get('key', nil)
      error(
        'Publisher account not authenticated.\n' + 
        'Run "nyssa account create" or "nyssa account login" to get started.'
      )

    log.info('Checking for valid Nyssa package')
    var config = Config.from_dict(json.decode(file(config_file).read()))
    if !config.name or !config.version
      error('Invalid Nyssa package.')

    var source_name = '${state.name}_${config.name}_${config.version}.nyp'
    tmp_dest = os.join_paths(storage_dir, source_name)

    log.info('Packaging ${config.name}@${config.version}...')

    # make a backup that has no git.
    var tmp_root = os.join_paths(storage_dir, '.tmp')
    copy_to_tmp('.', tmp_root)
    var curr_dir = os.cwd()
    os.change_dir(tmp_root)

    if zip.compress(os.cwd(), tmp_dest) {
      os.change_dir(curr_dir)
      os.remove_dir(tmp_root, true)

      # set authentication headers
      log.info('Authenticating')

      # make the request
      log.info('Uploading ${config.name}@${config.version} to ${repo}...')
      var res = http.post('${repo}/api/create-package', {
        name: config.name,
        version: config.version,
        config: json.encode(config),
        source: file(tmp_dest),
        readme: file(readme_file).exists() ? file(readme_file).read() : nil,
      }, {
        'Nyssa-Publisher-Name': state.name,
        'Nyssa-Publisher-Key': state.key,
      })
      var body = json.decode(res.body.to_string())

      # delete the package source file
      log.info('Removing temporary files')
      file(tmp_dest).delete()

      if res.status == 200 {
        success('Successfully published ${config.name}@${config.version}!')
      } else {
        error('Publish failed for ${config.name}@${config.version}:\n  ${body.error}')
      }
    } else {

      file(tmp_dest).delete()
      error('Packaging failure')
    }
  } as e

  if tmp_dest file(tmp_dest).delete()

  if e {
    error(e.message)
  }
}
