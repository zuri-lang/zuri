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

var config_file = os.join_paths(os.cwd(), setup.CONFIG_FILE)

def italic(t) {
  return colors.text(t, colors.style.italic)
}

def empty() {
  return italic('  None')
}

def bold(t) {
  return colors.text('${t} ', colors.style.bold)
}

def green(t) {
  return colors.text(t, colors.text_color.green)
}

def title(t) {
  return green(bold(t)) + '\n' + green('~' * t.length())
}

def info(n, m) {
  if m != nil return bold(' • ${n}:') + (m ? m : italic('None'))
  return ' • ${n}'
}

def parse(parser) {
  parser.add_command(
    'info', 
    'Shows current project information'
  )
}

def run(value, options, success, error) {
  if !file(config_file).exists() 
    error('Not in a Nyssa project')
  
  var config = json.decode(file(config_file).read())
  if !is_dict(config) or !config.get('name', nil) or !config.get('version', nil)
    error('Not in a Nyssa project')

  config = Config.from_dict(config)

  catch {
    echo ''
      echo title('PACKAGE INFORMATION')
      echo info('Name', config.name)
      echo info('Version', config.version)
      echo info('Description', config.description)
      echo info('Homepage', config.homepage)
      echo info('Author', config.author)
      echo info('License', config.license)
      echo info('Tags', ', '.join(config.tags))
      echo ''
      echo title('DEPENDENCIES')
      if config.deps {
        for name, version in config.deps {
          echo info(name, version)
        }
      } else {
        echo empty()
      }
      echo ''
      echo title('SOURCES')
      if config.sources {
        for source in config.sources {
          echo info(source)
        }
      } else {
        echo empty()
      }
      echo ''
  } as e

  if e {
    error('Failed to retrieve package information for ${value}:\n  ${e.message}')
  }
}
