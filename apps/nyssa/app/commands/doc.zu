import args
import os
import ..setup

var config_file = os.join_paths(os.cwd(), setup.CONFIG_FILE)

def parse(parser) {
  parser.add_command(
    'doc', 
    'Generates documentation for the current project'
  ).add_option(
    'with-private',
    'Generate documentation for private modules',
    {
     short_name: 'p',
    }
  ).add_option(
    'with-stub',
    'Generate documentation for stub modules',
    {
     short_name: 's',
    }
  )
}

def run(value, options, success, error) {
  var with_private = options.get('with-private', false),
      with_stub = options.get('with-stub', false)

  if !file(config_file).exists()
    error('Not in a Nyssa project')
}
