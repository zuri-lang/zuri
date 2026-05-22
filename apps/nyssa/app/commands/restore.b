import args
import hash
import os
import colors
import http
import json
import io
import zip
import log
import ..setup
import ..config { Config }
import ..util { setup_cli }


var cache_dir = os.join_paths(setup.NYSSA_DIR, setup.CACHE_DIR)
if !os.dir_exists(cache_dir)
  os.create_dir(cache_dir)

var config_file = os.join_paths(os.cwd(), setup.CONFIG_FILE)

def parse(parser) {
  parser.add_command(
    'restore', 
    'Restores all project dependencies'
  ).add_option(
    'no-cache', 
    'disables the cache', 
    {
      short_name: 'x',
    }
  )
}

def configure(config, repo, full_name, name, version, path, progress, no_cache, error) {
  log.info('Installing ${full_name}')

  var zuri_exe = os.args[0],
      destination = os.join_paths(os.cwd(), '.zuri/libs/${name}')

  # create the packages directory if not exists
  log.info('Creating package directory for ${full_name}')
  if os.dir_exists(destination)
    os.remove_dir(destination, true)
  os.create_dir(destination)

  # extract
  log.info('Extracting artefact for ${full_name}')
  if zip.extract(path, destination) {
    # add to list of installed packages
    progress.set(name, version)

    var package_config_file = os.join_paths(destination, setup.CONFIG_FILE)
    var package_config = Config.from_dict(json.decode(file(package_config_file).read()))

    # restore dependencies before running post-install in case post-install
    # depends on a dependency.
    if package_config.deps {
      echo ''
      log.info('Fetching dependencies for ${full_name}...')
      echo '--------------------------${"-" * full_name.length()}---'

      for dep, ver in package_config.deps {
        var dep_full_name = ver ? '${dep}@${ver}' : dep
        if !progress.contains(dep) {
          install(config, repo, dep_full_name, dep, ver, progress, no_cache, error)
        }
      }
    }

    # run post install script if it exists
    if package_config.post_install {
      log.info('Running post install for ${full_name}')

      # cd into the destination before running post_install so 
      # that post_install will run relative to the package.
      var this_dir = os.cwd()
      os.change_dir(destination)

      # run the script
      os.exec('${zuri_exe} ${package_config.post_install}')

      # return to current directory
      os.change_dir(this_dir)
    }

    # create cli if required
    if package_config.cli {
      var cli_path = os.join_paths(destination, package_config.cli)
      log.info('Creating CLI for ${name} at ${cli_path}')
      setup_cli(name, destination, cli_path)
    }

    return true
  } else {
    log.debug('${name} installation failed:\n  Failed to extract package source')
    return false
  }
}

def install(config, repo, full_name, name, version, progress, no_cache, error) {
  if !no_cache log.info('Checking local cache for ${full_name}')
  var cache_id = hash.sha1(repo + name + version)
  var cache_path = os.join_paths(cache_dir, '${cache_id}.nyp')

  catch {

    # check local cache first to avoid redownloading all the time...
    if file(cache_path).exists() and !no_cache {
      return configure(config, repo, full_name, name, nil, cache_path, progress, no_cache, error)
    }

    # fresh install
    log.info('>>> Fetching package metadata for ${full_name}')
    var res = http.get('${repo}/api/get-package/${full_name}')
    var body = json.decode(res.body.to_string())

    if res.status == 200 {
      log.info('${name} dependency ${body.name}@${body.version} found')

      # download source
      log.info('Downloading package source for ${full_name}...')
      var download_url = repo + '/source/' + body.source
      var download_req = http.get(download_url)
      if download_req.status == 200 {
        # save the file to cache
        log.info('Caching download for ${full_name}')
        file(cache_path, 'wb').write(download_req.body)

        # do package configuration
        return configure(config, repo, full_name, body.name, body.version, cache_path, progress, no_cache, error)
      } else {
        log.debug('${full_name} source not found at ${repo}')
        return false
      }
    } else {
      log.debug('${full_name} not found at ${repo}')
      return false
    }
  } as e

  if e {
    log.debug('${full_name} installation failed:\n  ${e.message}')
    return false
  }
}

def run(value, options, success, error) {
  if !file(config_file).exists() 
    error('Not in a Nyssa project')
  
  var config = json.decode(file(config_file).read())
  if !is_dict(config) or !config.get('name', nil) or !config.get('version', nil)
    error('Not in a Nyssa project')

  config = Config.from_dict(config)

  if config.deps {
    var progress = {}, 
        not_installed = [],
        no_cache = options.get('no-cache', false)

    for name, version in config.deps {
      var full_name = version ? '${name}@${version}' : name

      # do installation
      var installed = false
      for repo in config.sources {
        if install(config, repo, full_name, name, version, progress, no_cache, error) {
          installed = true
          break
        }
      }

      if !installed not_installed.append(full_name)
    }

    if not_installed {
      var error_msg = 'Could not install the following packages:'
      for nn in not_installed {
        error_msg += '\n  - ${nn}'
      }
      error(error_msg)
    }
  }

  success('Project ${config.name} dependencies successfully restored!')
}
